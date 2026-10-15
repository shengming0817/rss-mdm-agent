// @generated from execution-ipc::host::{ServiceView, RegistrationStatus}. Do not edit.

/**
 * Closed presentation combining local installation checks and authenticated service facts.
 */
export type ServiceView =
  | {
      phase: "notInstalled";
    }
  | {
      phase: "configurationRequired";
    }
  | {
      phase: "rejected";
    }
  | {
      phase: "unavailable";
    }
  | {
      phase: "mismatch";
    }
  | {
      phase: "connected";
      status: ServiceStatus;
    };

/**
 * Safe system owner facts.
 */
export interface ServiceStatus {
  /**
   * Actual binary build version.
   */
  build: string;
  /**
   * Native service platform.
   */
  platform: string;
  /**
   * Actual local IPC version.
   */
  protocol: number;
  /**
   * Independent readiness, not authorization or an effect receipt.
   */
  readiness:
    | {
        phase: "ready";
      }
    | {
        phase: "registrationRequired";
      }
    | {
        phase: "notReady";
      };
}

/**
 * A bounded machine request preview; contains no key handles, secrets or certificate bodies.
 */
export interface RegistrationStatus {
  /**
   * Exact retained credential state, independent of a pending authorization.
   */
  credential: "absent" | "active" | "expired" | "revoked" | "retired";
  /**
   * Original authorization deadline.
   */
  expiresAt?: number | null;
  /**
   * Server grant selected by the machine owner.
   */
  grantId?: string | null;
  /**
   * Closed lifecycle phase.
   */
  phase:
    | "unregistered"
    | "preparing"
    | "awaitingAuthorization"
    | "authorized"
    | "activating"
    | "active"
    | "issuanceUnknown"
    | "activationUnknown"
    | "paused"
    | "rejected";
  /**
   * SHA256 of the exact request public key.
   */
  publicKeyFingerprint?: string | null;
  /**
   * A durable registration exists, independent of runtime readiness.
   */
  registered: boolean;
}
