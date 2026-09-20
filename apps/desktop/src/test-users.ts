import { shallowRef } from "vue";
import { invoke, isTauri } from "@tauri-apps/api/core";
import {
  boundedJson,
  decode,
  accessLimits,
  type TestUserPage,
  type UserContext,
} from "@rss-mdm-agent/ai-contract";
export const currentUser = shallowRef<UserContext>();
export const nativeTestMode = isTauri();
function parsed<T extends "testUserPage" | "userContext">(
  input: unknown,
  kind: T,
): Extract<TestUserPage | UserContext, { kind: T }> {
  const record = decode(boundedJson(input, accessLimits), accessLimits);
  if (record.kind !== kind) throw new Error("invalid_response");
  return record as Extract<TestUserPage | UserContext, { kind: T }>;
}
export async function loadTestUsers(): Promise<TestUserPage> {
  const page = parsed(await invoke("test_users"), "testUserPage");
  currentUser.value = page.current;
  return page;
}
export async function selectTestUser(name: string): Promise<UserContext> {
  const context = parsed(
    await invoke("select_test_user", { name }),
    "userContext",
  );
  currentUser.value = context;
  return context;
}
export function userGeneration(): string {
  if (!currentUser.value) throw new Error("user_required");
  return currentUser.value.generation;
}
export async function enterCredential(): Promise<string> {
  const generation = userGeneration();
  const reference = await invoke<string>("enter_connection_credential", {
    generation,
  });
  if (generation !== userGeneration()) throw new Error("user_changed");
  return reference;
}

export async function discardCredential(
  reference: string,
  generation: string,
): Promise<void> {
  await invoke("discard_connection_credential", { reference, generation });
}

export function selectionMessage(error: unknown): string {
  const code =
    error && typeof error === "object" && "code" in error
      ? error.code
      : undefined;
  switch (code) {
    case "invalid_name":
      return "用户名需为 1–64 个字符，不能含控制字符；请修改后重试。";
    case "limit":
      return "测试用户数量已达上限，请选择已有用户。";
    case "users_unavailable":
      return "用户记录无法保存或读取。当前用户未切换，请检查本地存储后重试。";
    case "ai_unavailable":
      return "旧用户的 AI 请求尚未完成隔离，当前用户未切换。请重新连接 AI 后重试。";
    default:
      return "切换未完成，当前用户未变。请重新连接并重试；已登记设备任务仍可查询。";
  }
}
