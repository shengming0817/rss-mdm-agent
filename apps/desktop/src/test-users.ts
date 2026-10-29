import { proxy } from "valtio/vanilla";
import { invoke, isTauri } from "@tauri-apps/api/core";
import { boundedJson, decode, accessLimits } from "@rss-mdm-agent/ai-contract";
import type {
  TestUserPage,
  Connection,
  ConnectionDraft,
  UserContext,
  AccountSettings,
  AccountStatus,
  AccountOrganization,
  AccountFailure,
} from "@rss-mdm-agent/ai-contract/wire";
import type { Result } from "@rss-mdm-agent/ai-contract/ports";
export const accountState = proxy({
  user: undefined as UserContext | undefined,
  notice: "",
  binding:
    "unbound" as import("@rss-mdm-agent/ai-contract/wire").AccountBindingState,
  busy: false,
});
let accountEpoch = 0,
  readTicket = 0;
export function retireAccountOperations() {
  accountEpoch++;
  readTicket++;
  accountState.busy = false;
  accountState.user = undefined;
  accountState.notice = "";
  accountState.binding = "unbound";
}
/** Captures the application/account lifetime before a native asynchronous transition. */
export function accountScope() {
  const epoch = accountEpoch;
  return () => epoch === accountEpoch;
}
/** Invalidates passive reads around an authenticated registration transition. */
export function invalidateAccountReads() {
  readTicket++;
}
async function changeAccount<T>(
  run: (accept: () => boolean) => Promise<T>,
): Promise<T> {
  if (accountState.busy) throw new Error("account_busy");
  const epoch = ++accountEpoch;
  readTicket++;
  const accept = () => epoch === accountEpoch;
  accountState.busy = true;
  accountState.user = undefined;
  accountState.notice = "";
  try {
    return await run(accept);
  } finally {
    if (accept()) accountState.busy = false;
  }
}
async function selectAccount(
  read: () => Promise<unknown>,
): Promise<UserContext> {
  return changeAccount(async (accept) => {
    try {
      const value = parsed(await read(), "userContext");
      if (!accept()) throw new Error("account_operation_retired");
      accountState.user = value;
      return value;
    } catch (error) {
      if (accept()) await refreshAccount(accept);
      throw error;
    }
  });
}
import type { EnterpriseLogin } from "./account/types";
import { fixtureSelected } from "./assembly";
export const nativeTestMode = isTauri() && !fixtureSelected;
function parsed<
  T extends
    | "testUserPage"
    | "userContext"
    | "accountSettings"
    | "accountStatus",
>(
  input: unknown,
  kind: T,
): Extract<
  TestUserPage | UserContext | AccountSettings | AccountStatus,
  { kind: T }
> {
  const record = decode(boundedJson(input, accessLimits), accessLimits);
  if (record.kind !== kind) throw new Error("invalid_response");
  return record as Extract<
    TestUserPage | UserContext | AccountSettings | AccountStatus,
    { kind: T }
  >;
}
export async function loadTestUsers(): Promise<TestUserPage> {
  const epoch = accountEpoch,
    expected = accountState.user?.generation;
  const page = parsed(await invoke("test_users"), "testUserPage");
  if (epoch === accountEpoch && expected === accountState.user?.generation)
    accountState.user = page.current;
  return page;
}
export function selectTestUser(name: string): Promise<UserContext> {
  return selectAccount(() => invoke("select_test_user", { name }));
}
export function userGeneration(): string {
  if (!accountState.user) throw new Error("user_required");
  return accountState.user.generation;
}
/** A secret crosses one native invocation and is never returned in connection metadata. */
export async function saveNativeConnection(
  connection: ConnectionDraft,
  expected: number | null,
  secret?: string,
): Promise<Connection> {
  const generation = userGeneration();
  const pending = invoke<Result<Connection>>("save_connection", {
    generation,
    input: connection,
    expected,
    secret: secret ?? null,
  });
  secret = undefined;
  const result = await pending;
  if (generation !== userGeneration()) throw new Error("user_changed");
  if (!result.ok) throw result.error;
  const record = decode(boundedJson(result.value, accessLimits), accessLimits);
  if (record.kind !== "connection") throw new Error("invalid_response");
  return record;
}

export function selectionMessage(error: unknown): string {
  const code =
    error && typeof error === "object" && "code" in error
      ? error.code
      : undefined;
  switch (code) {
    case "logout_unconfirmed":
      return "本机会话已退出，服务端注销未确认。请在企业账户中撤销会话，然后重试切换。";
    case "invalid_name":
      return "用户名需为 1–64 个字符，不能含控制字符；请修改后重试。";
    case "limit":
      return "测试用户数量已达上限，请选择已有用户。";
    case "users_unavailable":
      return "用户记录无法保存或读取，请检查本地存储后重试。";
    case "ai_unavailable":
      return "旧用户的 AI 请求尚未完成隔离，请重新连接 AI 后重试。";
    default:
      return "切换未完成，请重新连接并重试；已登记设备任务仍可查询。";
  }
}

export function enterGuest(): Promise<UserContext> {
  return selectAccount(() => invoke("select_device"));
}
export function loginEnterprise(input: EnterpriseLogin): Promise<UserContext> {
  return selectAccount(() => invoke("account_login", { input }));
}
export function recoverAccount(clear = false): Promise<void> {
  return changeAccount(async (accept) => {
    try {
      const value = await invoke("account_recover", { clear });
      if (accept())
        accountState.user = value ? parsed(value, "userContext") : undefined;
    } finally {
      if (accept()) await refreshAccount(accept);
    }
  });
}
export function logoutAccount(): Promise<void> {
  return changeAccount(async (accept) => {
    try {
      await invoke("account_logout");
    } finally {
      if (accept()) await refreshAccount(accept);
    }
  });
}
export async function refreshAccount(
  accept: () => boolean = () => true,
): Promise<boolean> {
  if (!accept()) return false;
  const expected = accountState.user?.generation;
  const epoch = accountEpoch,
    ticket = ++readTicket;
  const current = () =>
    accept() && epoch === accountEpoch && ticket === readTicket;
  try {
    const status = parsed(await invoke("account_status"), "accountStatus");
    if (!current() || accountState.user?.generation !== expected) return false;
    const value = status.current;
    accountState.binding = status.binding;
    if (status.failure)
      accountState.notice = accountFailureMessage(status.failure);
    if (
      !value &&
      !status.failure &&
      accountState.user?.identity.mode === "user_device"
    )
      accountState.notice =
        "企业会话已失效或无法完成在线核验。旧视图已关闭，请检查网络并重新登录。";
    accountState.user = value ? parsed(value, "userContext") : undefined;
    return true;
  } catch {
    if (
      current() &&
      accountState.user?.generation === expected &&
      accountState.user?.identity.mode === "user_device"
    ) {
      accountState.user = undefined;
      accountState.notice =
        "无法核验企业会话，旧视图已关闭。请检查网络、组织权限并重新登录。";
    }
  }
  return false;
}

export type Organization = AccountOrganization;
export async function loadAccountSettings(): Promise<AccountSettings> {
  return parsed(await invoke("account_organizations"), "accountSettings");
}
export async function saveOrganization(
  input: Organization,
): Promise<Organization> {
  const page = parsed(
    await invoke("account_save_organization", { input }),
    "accountSettings",
  );
  const saved = page.organizations.find((item) => item.id === page.selected);
  if (!saved) throw new Error("invalid_response");
  return saved;
}
export function accountFailureMessage(
  failure: Pick<AccountFailure, "stage" | "reason">,
): string {
  const stage =
    new Map([
      ["configuration", "组织配置"],
      ["login", "登录"],
      ["session", "会话核验"],
      ["authorization", "业务授权核验"],
      ["logout", "服务端注销"],
    ]).get(failure.stage) ?? "账户";
  const reason =
    new Map([
      ["configuration", "请检查组织服务地址和租户 UUID。"],
      ["denied", "账号或权限已失效，请重新登录或联系组织管理员。"],
      ["unavailable", "网络或服务暂不可用，请检查连接后重试。"],
      ["rate_limited", "请求受到限流，请稍后重试。"],
      ["contract", "服务响应不兼容，请核对服务版本或联系管理员。"],
    ]).get(failure.reason) ?? "账户操作未完成，请重试。";
  return `${stage}：${reason}`;
}
export function accountErrorMessage(error: unknown): string {
  const code = (error as { code?: string })?.code;
  if (code === "logout_unconfirmed")
    return "本机会话已退出，服务端注销未确认；请在企业账户中撤销会话后重试。";
  if (code === "default_organization")
    return "默认组织名称来自应用内置配置；请选择其它连接。";
  if (code === "cancelled") return "已取消登录";
  if (code === "execution_unavailable")
    return "本机执行服务未确认身份切换，请在设置中检查服务状态。";
  if (code === "current_user_unconfirmed")
    return "设备当前使用者尚未清理，请重试；本机工作区仍可使用。";
  if (typeof code === "string" && code.startsWith("account/")) {
    const [, stage, reason] = code.split("/");
    try {
      const status = parsed(
        {
          schemaVersion: 1,
          kind: "accountStatus",
          binding: "unbound",
          failure: { stage, reason, observedAtMs: 0 },
        },
        "accountStatus",
      );
      if (status.failure) return accountFailureMessage(status.failure);
    } catch {}
  }
  if (code === "registration_login_required")
    return "设备注册需要企业账号核验，请登录后重试。";
  if (code === "registration_organization_mismatch")
    return "当前账号与设备配置的组织不一致，请核对组织。";
  if (code === "registration_cancelled")
    return "已停止等待注册；已受理的设备状态仍需重新读取。";
  if (code === "binding_unknown") return "登录确认尚未完成，请重试原操作。";
  if (code === "binding_conflict")
    return "当前使用者发生冲突，请清理后重新登录。";
  if (code === "cleanup_pending") return "退出清理尚未确认，请重试原操作。";
  if (code === "binding_invalidated") return "设备身份已变化，请重新登录。";
  return "账户操作未完成，请重试；如持续失败，请联系管理员。";
}
