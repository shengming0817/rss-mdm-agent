// Desktop presentation DTOs. Rust owns encoding and all domain decisions.
export type Decision =
  | "unknown"
  | "allowed"
  | "missingCapability"
  | "unsupportedTarget"
  | "unresolvedResource"
  | "blocked";
export type ParameterRule =
  | {
      type: "string";
      minLength: number;
      maxLength: number;
      choices?: string[] | null;
      default?: string | null;
    }
  | {
      type: "integer";
      minimum: number;
      maximum: number;
      choices?: number[] | null;
      default?: number | null;
    }
  | { type: "boolean"; default?: boolean | null }
  | { type: "secretReference" };
export interface Parameter {
  title: string;
  description: string;
  required: boolean;
  rule: ParameterRule;
}
export interface CatalogRef {
  authority: { kind: "test"; id: string };
  identity: { id: string; revision: string };
  digest: string;
}
export interface Resource {
  reference: { id: string; revision: string };
  versionDigest: string;
  selector: { platform: string; architecture: string; key: string };
}
export interface CatalogItem {
  catalog: CatalogRef;
  itemId: string;
  variantId: string;
  kind: "software" | "script" | "tool";
  name: string;
  description: string;
  category: string;
  resource: Resource;
  fields: Record<string, Parameter>;
  inputSchema: unknown;
  display: {
    visibility: Decision;
    requestability: Decision;
    executability: Decision;
  };
  reason: string;
  availability: "listed" | "withdrawn" | "expired";
}
export type FieldInput =
  | { kind: "text" | "integer"; value: string }
  | { kind: "boolean"; value: boolean }
  | { kind: "secretReference"; id: string; revision: string };
export interface Draft {
  instanceId: string;
  requestId: string;
  revision: number;
  catalog: CatalogRef;
  itemId: string;
  variantId: string;
  fields: Record<string, FieldInput>;
}
export interface Plan {
  requestId: string;
  revision: number;
  planId: string;
  digest: string;
  itemId: string;
  title: string;
  action: string;
  resource: Resource;
  target: string;
  runAs: string;
  network: string;
  dataScope: string;
  permission: string;
  parameters: { label: string; state: string }[];
  expiresAtUnixMs: number;
}
export type InteractionKind =
  | { kind: "userConfirmation"; purpose: string }
  | { kind: "privacyConsent"; scope: string }
  | { kind: "administratorAuthorization"; request: string }
  | { kind: "parameterInput"; schema: string }
  | { kind: "maintenanceWindow" | "restartPrompt"; options: string };
export interface Interaction {
  id: string;
  kind: InteractionKind;
  status: "pending" | "answered" | "cancelled" | "expired";
  message: string;
  expiresAtUnixMs: number;
  options: { id: string; label: string }[];
}
export interface RequestView {
  plan: Plan;
  status:
    | "waiting"
    | "approval"
    | "complete"
    | "stopped"
    | "restartRequired"
    | "unknownEffect";
  message: string;
  interactions: Interaction[];
}
export interface Snapshot {
  mode: "fixture";
  instanceId: string;
  targetLabel: string;
  catalog: CatalogItem[];
  requests: RequestView[];
}
export interface Submission {
  instanceId: string;
  requestId: string;
  planId: string;
  digest: string;
}
export type Answer =
  | { kind: "confirmation" | "privacyConsent"; accepted: boolean }
  | { kind: "choice"; selection: string }
  | { kind: "parameters"; fields: Record<string, FieldInput> }
  | { kind: "cancel" };
export interface Reply {
  instanceId: string;
  requestId: string;
  interactionId: string;
  commandId: string;
  answer: Answer;
}
export interface SelfServicePort {
  snapshot(): Promise<Snapshot>;
  preview(input: Draft): Promise<Plan>;
  submit(input: Submission): Promise<RequestView>;
  respond(input: Reply): Promise<RequestView>;
}
