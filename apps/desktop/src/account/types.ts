import type {
  AccountOrganization,
  UserContext,
} from "@rss-mdm-agent/ai-contract/wire";
/** Native login is one atomic request, never a partial legacy argument list. */
export interface EnterpriseLogin {
  organizationId: string;
  login: string;
  password: string;
}
/** Only the test assembly supplies these synthetic alternative login capabilities. */
export interface FixtureLoginPort {
  state: {
    qr: "waiting" | "scanned" | "expired" | "denied";
    clients: Array<{
      id: string;
      name: string;
      kind: "IM" | "零信任";
      account: string;
      status: "signed_in" | "signed_out" | "expired" | "mismatch";
    }>;
    discovering: boolean;
    error: string;
    current?: UserContext;
  };
  organizations: AccountOrganization[];
  password(input: EnterpriseLogin): Promise<UserContext>;
  refreshQr(): void;
  scan(): void;
  expire(): void;
  reject(): void;
  confirm(): Promise<void>;
  discover(): Promise<void>;
  scenario(
    value:
      | "normal"
      | "signed_out"
      | "missing"
      | "mismatch"
      | "expired"
      | "error",
  ): void;
  loginClient(id: string): Promise<void>;
  cancel(): void;
  organization(id: string): void;
}
