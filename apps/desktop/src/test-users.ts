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
