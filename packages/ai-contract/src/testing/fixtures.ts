import data from "./fixtures.json" with { type: "json" };
import type { WireRecord } from "../wire.js";
import type { Diagnostic, Limits } from "../codec.js";
/** One shared golden set; this annotation keeps JSON compiler options out of consumer declarations. */
export const fixtures: {
  valid: WireRecord[];
  invalid: Array<{
    name: string;
    raw: string;
    code: Diagnostic;
    limits?: Partial<Limits>;
  }>;
  commandHash: string;
} = data as unknown as {
  valid: WireRecord[];
  invalid: Array<{
    name: string;
    raw: string;
    code: Diagnostic;
    limits?: Partial<Limits>;
  }>;
  commandHash: string;
};
