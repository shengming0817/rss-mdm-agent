import { proxy, ref, subscribe } from "valtio/vanilla";
import type { TestUser, UserContext } from "@rss-mdm-agent/ai-contract/wire";
import type { ThemePreference } from "@rss-mdm-agent/ui";
import type { EnterpriseLogin } from "../account/types";
import {
  accountState,
  accountErrorMessage,
  enterGuest,
  invalidateAccountReads,
  loadTestUsers,
  loginEnterprise,
  logoutAccount,
  nativeTestMode,
  recoverAccount,
  refreshAccount,
  retireAccountOperations,
  selectTestUser,
} from "../test-users";
import { nativeService } from "../settings/native";
import { createAppearance } from "../appearance";
import { workspaceIdentity } from "../workspace/identity";
import {
  createWorkspace,
  type Workspace,
  type WorkspaceInput,
} from "./workspace";

/** The application owns account observation; workspace owners never share a client. */
export function createDesktop(input: WorkspaceInput = {}) {
  if (input.environment && !import.meta.env.DEV)
    throw Error("fixture is development-only");
  const state = proxy({
    phase: "starting" as
      | "starting"
      | "ready"
      | "signedOut"
      | "unavailable"
      | "failed",
    workspace: undefined as ReturnType<typeof ref<Workspace>> | undefined,
    users: [] as TestUser[],
    error: "",
    registrationPending: false,
    theme: "system" as ThemePreference,
  });
  const appearance = input.environment
    ? createAppearance(async () => ({
        materialEnabled: false,
        reducedMotion: false,
        highContrast: false,
      }))
    : createAppearance();
  const service = input.environment
    ? input.environment.service
    : nativeService();
  let alive = true,
    started = false,
    bootstrapped = false,
    reading = false,
    epoch = 0,
    registrationEpoch = 0;
  let polling: ReturnType<typeof setInterval> | undefined;
  function dropWorkspace() {
    const previous = state.workspace;
    state.workspace = undefined;
    previous?.dispose();
  }
  function replaceWorkspace(user?: UserContext) {
    if (
      !alive ||
      (!input.environment && accountState.busy) ||
      state.registrationPending
    ) {
      dropWorkspace();
      return;
    }
    if (state.workspace?.key === workspaceIdentity(user)) return;
    dropWorkspace();
    if (!user) {
      if (state.phase !== "starting") state.phase = "signedOut";
      return;
    }
    const workspace = createWorkspace(user, input);
    state.workspace = ref(workspace);
    state.phase = "ready";
    workspace.start();
  }
  const stopAccount = input.environment
    ? () => {}
    : subscribe(
        accountState,
        () => {
          if (bootstrapped) replaceWorkspace(accountState.user);
        },
        true,
      );
  async function pollAccount() {
    if (
      !alive ||
      input.environment ||
      state.phase === "starting" ||
      accountState.busy ||
      state.registrationPending ||
      !accountState.user ||
      document.hidden ||
      reading
    )
      return;
    reading = true;
    const current = epoch,
      registration = registrationEpoch;
    try {
      await refreshAccount(
        () =>
          alive &&
          current === epoch &&
          registration === registrationEpoch &&
          !state.registrationPending,
      );
    } finally {
      reading = false;
    }
  }
  const visibility = () => {
    if (!document.hidden) {
      void pollAccount();
      void appearance.refresh();
    }
  };
  async function start() {
    if (started || !alive) return;
    started = true;
    const current = ++epoch;
    void appearance.refresh();
    if (!input.environment && !nativeTestMode) {
      state.phase = "unavailable";
      return;
    }
    try {
      if (input.environment) {
        const user = await input.environment.identity.read();
        if (alive && current === epoch) {
          bootstrapped = true;
          replaceWorkspace(user);
        }
      } else {
        const page = await loadTestUsers();
        if (!alive || current !== epoch) return;
        state.users = page.users;
        const verified = await refreshAccount(() => alive && current === epoch);
        if (!alive || current !== epoch) return;
        if (!verified) throw Error("account_unverified");
        if (
          !accountState.user &&
          !["cleanup_pending", "conflict"].includes(accountState.binding)
        )
          await enterGuest();
        if (!alive || current !== epoch) return;
        bootstrapped = true;
        replaceWorkspace(accountState.user);
      }
    } catch (error) {
      if (alive && current === epoch) {
        state.error = accountErrorMessage(error);
        state.phase = "failed";
      }
    } finally {
      if (alive && current === epoch) {
        if (state.phase === "starting")
          state.phase = state.workspace ? "ready" : "signedOut";
        polling = setInterval(() => void pollAccount(), 2000);
        document.addEventListener("visibilitychange", visibility);
      }
    }
  }
  async function accountAction(action: () => Promise<unknown>) {
    if (
      !alive ||
      input.environment ||
      !bootstrapped ||
      accountState.busy ||
      state.registrationPending
    )
      return;
    state.error = "";
    try {
      await action();
    } catch (error) {
      if (alive) state.error = accountErrorMessage(error);
    }
    if (alive) replaceWorkspace(accountState.user);
  }
  async function registrationAction<T>(run: () => Promise<T>) {
    if (!alive || accountState.busy || state.registrationPending)
      throw Error("account_busy");
    registrationEpoch++;
    state.registrationPending = true;
    invalidateAccountReads();
    dropWorkspace();
    try {
      return await run();
    } finally {
      if (alive) {
        if (!input.environment) await refreshAccount(() => alive);
        state.registrationPending = false;
        const user = input.environment
          ? await input.environment.identity.read()
          : accountState.user;
        if (alive) replaceWorkspace(user);
      }
    }
  }
  const servicePort = service && {
    ...service,
    requestRegistration: (login?: EnterpriseLogin) =>
      registrationAction(() => service.requestRegistration(login)),
    importRegistration: () =>
      registrationAction(() => service.importRegistration()),
  };
  return {
    state,
    appearance,
    account: accountState,
    service: servicePort,
    start,
    pollAccount,
    login: (input: EnterpriseLogin) =>
      accountAction(() => loginEnterprise(input)),
    guest: () => accountAction(enterGuest),
    selectUser: (name: string) => accountAction(() => selectTestUser(name)),
    logout: () => accountAction(logoutAccount),
    recover: (clear = false) => accountAction(() => recoverAccount(clear)),
    setTheme(theme: ThemePreference) {
      if (alive) state.theme = theme;
    },
    dispose() {
      if (!alive) return;
      alive = false;
      epoch++;
      registrationEpoch++;
      clearInterval(polling);
      document.removeEventListener("visibilitychange", visibility);
      stopAccount();
      dropWorkspace();
      appearance.dispose();
      if (!input.environment) retireAccountOperations();
      state.users = [];
      state.error = "";
    },
  };
}
export type Desktop = ReturnType<typeof createDesktop>;
