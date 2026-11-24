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
   * Installation owner readback, absent when observation is unavailable.
   */
  installation?: InstallationObservation | null;
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
        phase: "notReady";
      };
  /**
   * Bounded local telemetry health; diagnostic values grant no authority.
   */
  telemetry?: {
    [k: string]: unknown;
  };
}
/**
 * Non-authorizing observations sampled by the protected installation service.
 */
export interface InstallationObservation {
  /**
   * Closed diagnostic from the installation profile journal, if the operation failed.
   */
  applicationFailure?: string | null;
  /**
   * The DA store's last confirmed registration state.
   */
  daRegistration:
    | "registered"
    | "unregistered"
    | "expired"
    | "revoked"
    | "unknown";
  /**
   * The MDM agent store's last confirmed registration state.
   */
  mdmRegistration:
    | "registered"
    | "unregistered"
    | "expired"
    | "revoked"
    | "unknown";
  modules: ClientModules;
  /**
   * Device observation time, UTC seconds.
   */
  observedAt: number;
}
/**
 * Installed shared service, administrator selection and profile operation outcome.
 */
export interface ClientModules {
  /**
   * Installation-profile outcome, independent of native login readiness.
   */
  application: "unknown" | "received" | "pending" | "applied" | "rejected";
  da: ModuleObservation;
  mdm: ModuleObservation1;
}
/**
 * Device Access installation and enablement.
 */
export interface ModuleObservation {
  /**
   * Administrator's local module selection.
   */
  enabled?: boolean | null;
  /**
   * Installed components observed on disk.
   */
  installed?: boolean | null;
}
/**
 * MDM installation and enablement.
 */
export interface ModuleObservation1 {
  /**
   * Administrator's local module selection.
   */
  enabled?: boolean | null;
  /**
   * Installed components observed on disk.
   */
  installed?: boolean | null;
}

/**
 * Closed, credential-free registration diagnostics.
 */
export type RegistrationFailure =
  | "invalidConfiguration"
  | "conflict"
  | "denied"
  | "expired"
  | "capacity"
  | "unavailable";

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
   * Classified failure without untrusted server text or secrets.
   */
  failure?: RegistrationFailure | null;
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
    | "configurationUnknown"
    | "awaitingAuthorization"
    | "authorized"
    | "activating"
    | "active"
    | "issuanceUnknown"
    | "activationUnknown"
    | "paused"
    | "rejected";
  /**
   * Last Agent PKI installation observation; not a CRL freshness proof.
   */
  pkiTrust?: TrustInstallationStatus | null;
  /**
   * SHA256 of the exact request public key.
   */
  publicKeyFingerprint?: string | null;
  /**
   * A durable registration exists, independent of runtime readiness.
   */
  registered: boolean;
  /**
   * Last task trust observation; absence means unknown.
   */
  taskTrust?: TrustInstallationStatus | null;
}
/**
 * Non-secret installation progress from the sole communication owner.
 */
export interface TrustInstallationStatus {
  /**
   * Separate successful ACK confirmation.
   */
  acknowledged: boolean;
  /**
   * Last read-back installed public revision when known.
   */
  actualRevision?: number | null;
  /**
   * Local observation time, not ordering authority.
   */
  observedAt: number;
  /**
   * Last locally observed result; not current connectivity.
   */
  state: "received" | "applied" | "failed" | "unknown";
  /**
   * Target public revision when known.
   */
  targetRevision?: number | null;
}

/**
 * Read-only native state, independent from Agent identity and policy authorization.
 */
export interface MdmStatus {
  /**
   * UTC observation time, absent before the first successful owner drive.
   */
  observedAt?: number | null;
  /**
   * Local OS observation; Unknown does not mean absent.
   */
  state: "unenrolled" | "this_organization" | "other_organization" | "unknown";
}
