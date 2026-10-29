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
  const account = input.environment
    ? proxy({
        user: undefined as UserContext | undefined,
        notice: "",
        binding: "unbound" as typeof accountState.binding,
        busy: false,
      })
    : accountState;
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
    if (!alive || account.busy || state.registrationPending) {
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
  const stopAccount = subscribe(
    account,
    () => {
      if (bootstrapped) replaceWorkspace(account.user);
    },
    true,
  );
  async function pollAccount() {
    if (
      !alive ||
      input.environment ||
      state.phase === "starting" ||
      account.busy ||
      state.registrationPending ||
      !account.user ||
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
    state.phase = "starting";
    state.error = "";
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
          account.user = user;
          account.binding =
            user.identity.mode === "user_device" ? "bound" : "unbound";
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
          !account.user &&
          !["cleanup_pending", "conflict"].includes(account.binding)
        )
          await enterGuest();
        if (!alive || current !== epoch) return;
        bootstrapped = true;
        replaceWorkspace(account.user);
      }
    } catch (error) {
      if (alive && current === epoch) {
        started = false;
        state.error = accountErrorMessage(error);
        state.phase = "failed";
      }
    } finally {
      if (alive && bootstrapped && current === epoch) {
        if (state.phase === "starting")
          state.phase = state.workspace ? "ready" : "signedOut";
        polling = setInterval(() => void pollAccount(), 2000);
        document.addEventListener("visibilitychange", visibility);
      }
    }
  }
  async function accountAction(
    action: () => Promise<unknown>,
    fixtureAction?: () => Promise<unknown>,
  ): Promise<boolean> {
    if (!alive || !bootstrapped || account.busy || state.registrationPending)
      return false;
    state.error = "";
    if (input.environment) {
      if (!fixtureAction) return false;
      const current = ++epoch;
      account.busy = true;
      account.user = undefined;
      dropWorkspace();
      try {
        await fixtureAction();
        const user = await input.environment.identity.read();
        if (!alive || current !== epoch) return false;
        account.user = user;
        account.binding =
          user.identity.mode === "user_device" ? "bound" : "unbound";
        return true;
      } catch (error) {
        if (alive && current === epoch)
          state.error = accountErrorMessage(error);
        return false;
      } finally {
        if (alive && current === epoch) {
          account.busy = false;
          replaceWorkspace(account.user);
        }
      }
    }
    try {
      await action();
      if (!alive) return false;
      replaceWorkspace(account.user);
      return true;
    } catch (error) {
      if (alive) {
        state.error = accountErrorMessage(error);
        replaceWorkspace(account.user);
      }
      return false;
    }
  }
  async function registrationAction<T>(run: () => Promise<T>) {
    if (!alive || account.busy || state.registrationPending)
      throw Error("account_busy");
    registrationEpoch++;
    state.registrationPending = true;
    state.error = "";
    invalidateAccountReads();
    dropWorkspace();
    try {
      return await run();
    } catch (error) {
      if (alive) state.error = accountErrorMessage(error);
      throw error;
    } finally {
      if (alive) {
        if (!input.environment) await refreshAccount(() => alive);
        state.registrationPending = false;
        const user = input.environment
          ? await input.environment.identity.read()
          : account.user;
        if (alive) {
          if (input.environment) account.user = user;
          replaceWorkspace(user);
          state.workspace?.navigation.select("settings", "registration");
        }
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
    fixture: input.environment,
    appearance,
    account,
    service: servicePort,
    start,
    pollAccount,
    login: (credentials: EnterpriseLogin) =>
      accountAction(
        () => loginEnterprise(credentials),
        () => input.environment!.account!.password(credentials),
      ),
    guest: () =>
      accountAction(enterGuest, () => input.environment!.identity.guest()),
    selectUser: (name: string) => accountAction(() => selectTestUser(name)),
    logout: () =>
      accountAction(logoutAccount, () => input.environment!.identity.guest()),
    recover: (clear = false) =>
      accountAction(
        () => recoverAccount(clear),
        () => input.environment!.identity.guest(),
      ),
    confirmQr: input.environment?.account
      ? () =>
          accountAction(
            async () => {},
            async () => {
              await input.environment!.account!.confirm();
              if (!input.environment!.account!.state.current)
                throw Error("login_cancelled");
            },
          )
      : undefined,
    loginClient: input.environment?.account
      ? (id: string) =>
          accountAction(
            async () => {},
            async () => {
              await input.environment!.account!.loginClient(id);
              if (!input.environment!.account!.state.current)
                throw Error("login_cancelled");
            },
          )
      : undefined,
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
      input.environment?.account?.cancel();
      if (input.environment) {
        account.user = undefined;
        account.busy = false;
      }
      state.users = [];
      state.error = "";
    },
  };
}
export type Desktop = ReturnType<typeof createDesktop>;
