// @generated from execution-runner::host::ServiceView. Do not edit.

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
