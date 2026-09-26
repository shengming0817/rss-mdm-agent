import { shallowRef } from "vue";
import { invoke, isTauri } from "@tauri-apps/api/core";
import {
  boundedJson,
  decode,
  accessLimits,
  type TestUserPage,
  type Connection,
  type Result,
  type UserContext,
  type AccountSettings,
  type AccountStatus,
  type AccountOrganization,
  type AccountFailure,
} from "@rss-mdm-agent/ai-contract";
export const currentUser = shallowRef<UserContext>();
export const accountNotice = shallowRef("");
export const nativeTestMode = isTauri();
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
  const page = parsed(await invoke("test_users"), "testUserPage");
  currentUser.value = page.current;
  return page;
}
export async function selectTestUser(name: string): Promise<UserContext> {
  try {
    const context = parsed(
      await invoke("select_test_user", { name }),
      "userContext",
    );
    currentUser.value = context;
    return context;
  } catch (error) {
    await refreshAccount();
    throw error;
  }
}
export function userGeneration(): string {
  if (!currentUser.value) throw new Error("user_required");
  return currentUser.value.generation;
}
/** Native input stays inside the native save operation; only public metadata returns. */
export async function saveNativeConnection(
  connection: Connection,
  expected: number | null,
  replaceKey: boolean,
): Promise<Connection> {
  const generation = userGeneration();
  const result = await invoke<Result<Connection>>("save_connection", {
    generation,
    input: connection,
    expected,
    replaceKey,
  });
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

export async function enterGuest(): Promise<UserContext> {
  try {
    const context = parsed(await invoke("select_guest"), "userContext");
    currentUser.value = context;
    return context;
  } catch (error) {
    await refreshAccount();
    throw error;
  }
}
export async function loginEnterprise(
  organizationId: string,
  login: string,
): Promise<UserContext> {
  try {
    const context = parsed(
      await invoke("account_login", { organizationId, login }),
      "userContext",
    );
    currentUser.value = context;
    return context;
  } catch (error) {
    // Native login revokes the preceding account before changing organizations.
    await refreshAccount();
    throw error;
  }
}
export async function logoutAccount(): Promise<void> {
  try {
    await invoke("account_logout");
  } finally {
    currentUser.value = undefined;
  }
}
export async function refreshAccount(): Promise<void> {
  const expected = currentUser.value?.generation;
  try {
    const status = parsed(await invoke("account_status"), "accountStatus");
    const value = status.current;
    if (currentUser.value?.generation !== expected) return;
    if (status.failure)
      accountNotice.value = accountFailureMessage(status.failure);
    if (
      !value &&
      !status.failure &&
      currentUser.value?.identity?.mode === "enterprise"
    )
      accountNotice.value =
        "企业会话已失效或无法完成在线核验。旧视图已关闭，请检查网络并重新登录。";
    currentUser.value = value ? parsed(value, "userContext") : undefined;
  } catch {
    if (
      currentUser.value?.generation === expected &&
      currentUser.value?.identity?.mode === "enterprise"
    ) {
      currentUser.value = undefined;
      accountNotice.value =
        "无法核验企业会话，旧视图已关闭。请检查网络、组织权限并重新登录。";
    }
  }
}

export type Organization = AccountOrganization;
export async function loadOrganizations(): Promise<Organization[]> {
  return parsed(await invoke("account_organizations"), "accountSettings")
    .organizations;
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
  if (code === "cancelled") return "已取消登录";
  if (typeof code === "string" && code.startsWith("account/")) {
    const [, stage, reason] = code.split("/");
    try {
      const status = parsed(
        {
          schemaVersion: 5,
          kind: "accountStatus",
          failure: { stage, reason, observedAtMs: 0 },
        },
        "accountStatus",
      );
      if (status.failure) return accountFailureMessage(status.failure);
    } catch {}
  }
  return "账户操作未完成，请重试；如持续失败，请联系管理员。";
}
