import {
  ClientError,
  type ClientErrorCode,
  type InteractionView,
} from "@rss-mdm-agent/ai-client";
import {
  RuntimeSurface,
  type RendererFactory,
  type SurfaceRendererHandle,
  type RendererError,
} from "@rss-mdm-agent/ai-ui-bridge";
import type { Receipt } from "@rss-mdm-agent/ai-contract";
const handle: SurfaceRendererHandle = {
  canRetry: false,
  replace() {},
  clear() {},
  dispose() {},
  async retry() {},
};
export const factory: RendererFactory = async () => handle;
type Props = InstanceType<typeof RuntimeSurface>["$props"];
type NotAny<T> = 0 extends 1 & T ? false : true;
type Assert<T extends true> = T;
export type TypedError = Assert<
  NotAny<Parameters<NonNullable<Props["onError"]>>[0]>
>;
export type TypedReceipt = Assert<
  NotAny<Parameters<NonNullable<Props["onReceipt"]>>[0]>
>;
export const handlers: Pick<Props, "onError" | "onReceipt"> = {
  onError: (error: RendererError) => {
    const code: string = error.code;
    void code;
    const message: string = error.message;
    void message;
  },
  onReceipt: (receipt: Receipt) => {
    const kind: "receipt" = receipt.kind;
    void kind;
  },
};

export function recovery(
  error: ClientError,
  interaction: InteractionView,
): ClientErrorCode | number {
  return error.code === "expired" ? interaction.expiresAtMs : error.code;
}
