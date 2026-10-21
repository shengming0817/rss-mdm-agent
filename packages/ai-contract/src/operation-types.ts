// @generated from schema/operation.schema.json. Do not edit.

export type OperationContract =
  | OperationPresentation
  | OperationSnapshot
  | OperationEvent;
export type OperationValue = (string | number | boolean | null) | string[];

export interface OperationPresentation {
  version: 1;
  template:
    | "SettingsForm"
    | "ObjectActions"
    | "ResourceSelection"
    | "ChangeReview"
    | "TaskExecution"
    | "GuidedFlow"
    | "OperationApproval";
  /**
   * @maxItems 128
   */
  fields: OperationField[];
  /**
   * @maxItems 128
   */
  rules: OperationRule[];
  /**
   * @maxItems 16
   */
  sections: {
    id: string;
    label: string;
  }[];
  layout?: "sections" | "tabs" | "disclosure";
  selection?: "single" | "multiple";
  /**
   * @maxItems 16
   */
  steps?: {
    id: string;
    label: string;
    /**
     * @maxItems 128
     */
    fields: string[];
  }[];
  [k: string]: unknown;
}
export interface OperationField {
  id: string;
  label: string;
  component:
    | "TextField"
    | "NumberField"
    | "SelectField"
    | "Switch"
    | "Checkbox"
    | "RadioGroup"
    | "PathPicker"
    | "DateTimeField"
    | "SecretField";
  required?: boolean;
  readonly?: boolean;
  reason?: string;
  min?: number;
  max?: number;
  maxLength?: number;
  /**
   * @maxItems 128
   */
  options?: {
    id: string;
    label: string;
    disabledReason?: string;
  }[];
  submitMode?: "draft" | "immediate";
  section?: string;
  [k: string]: unknown;
}
export interface OperationRule {
  field: string;
  when: string;
  equals: string | number | boolean | null;
  kind: "required" | "equals";
  message: string;
}
export interface OperationSnapshot {
  scope: OperationScope;
  revision: number;
  baseline: string;
  current: {
    [k: string]: OperationValue;
  };
  proposal: {
    [k: string]: OperationValue;
  };
  phase:
    | "ready"
    | "submitting"
    | "waiting"
    | "running"
    | "execution_ended"
    | "failed"
    | "unknown"
    | "succeeded"
    | "pending_effect"
    | "cancelled"
    | "denied";
  reason?: string;
  /**
   * @maxItems 4
   */
  allowed: ("submit" | "cancel" | "refresh" | "answer")[];
  approval: {
    user: string;
    enterprise: string;
    elevation: string;
  };
  /**
   * @maxItems 128
   */
  objects: OperationObject[];
  /**
   * @maxItems 200
   */
  logs: string[];
  progress?: {
    completed: number;
    total: number;
  };
  result?: string;
  interaction?: OperationInteraction;
  expiresAtMs?: number;
  presentation: {
    title: string;
    targetLabel: string;
    source: string;
    impact: string;
    [k: string]: unknown;
  };
  /**
   * @maxItems 128
   */
  secretFields?: string[];
}
export interface OperationScope {
  authority: string;
  tenant: string;
  principal: string;
  session: string;
  target: string;
  instance: string;
}
export interface OperationObject {
  id: string;
  label: string;
  detail: string;
  disabledReason?: string;
  status?: string;
  [k: string]: unknown;
}
export interface OperationInteraction {
  id: string;
  question: string;
  /**
   * @maxItems 16
   */
  options: {
    id: string;
    label: string;
  }[];
  expiresAtMs: number;
}
export interface OperationEvent {
  id: string;
  instance: string;
  revision: number;
  kind: "submit" | "cancel" | "refresh" | "answer";
  values: {
    [k: string]: OperationValue;
  };
  /**
   * @maxItems 128
   */
  selection: string[];
  secretRefs: {
    [k: string]: string;
  };
  interactionId?: string;
  answer?: string;
}
