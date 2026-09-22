// @generated from local-service::ServiceView. Do not edit.

/**
 * Native-only projection. Connected is not an authorization to execute work.
 */
export type ServiceView =
  | {
      phase: "notInstalled";
      [k: string]: unknown;
    }
  | {
      phase: "rejected";
      [k: string]: unknown;
    }
  | {
      phase: "unavailable";
      [k: string]: unknown;
    }
  | {
      phase: "connected";
      status: Status;
      [k: string]: unknown;
    };
export type Capability = "statusOnly";

/**
 * A service statement, never an execution authorization or device receipt.
 */
export interface Status {
  build: string;
  capability: Capability;
  installation: string;
  platform: string;
  version: number;
}
