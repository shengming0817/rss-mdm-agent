import { RequestError } from "@agentclientprotocol/sdk";
import { errorCodes, type ErrorCode } from "@rss-mdm-agent/ai-contract";

export type ClientErrorCode =
  | ErrorCode
  | "not_initialized"
  | "negotiation_failed"
  | "a2ui_not_negotiated"
  | "invalid_response"
  | "resync_required"
  | "restore_superseded"
  | "transport_closed"
  | "transport_failed"
  | "transport_capacity"
  | "invalid_transport_message"
  | "request_failed";
/** Closed and value-free: provider messages, data and causes never cross this boundary. */
export class ClientError extends Error {
  constructor(readonly code: ClientErrorCode) {
    super(
      code === "a2ui_not_negotiated"
        ? "A2UI not negotiated"
        : `AI client: ${code}`,
    );
    this.name = "ClientError";
  }
}
export function clientError(
  error: unknown,
  fallback: ClientErrorCode,
): ClientError {
  if (error instanceof ClientError) return error;
  if (error instanceof RequestError) {
    const code = (error.data as { code?: unknown } | undefined)?.code;
    if (typeof code === "string" && errorCodes.some((known) => known === code))
      return new ClientError(code as ErrorCode);
  }
  return new ClientError(fallback);
}
