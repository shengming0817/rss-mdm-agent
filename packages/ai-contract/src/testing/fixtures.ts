import data from "./fixtures.json" with { type: "json" };
import type { WireRecord } from "../wire.js";
import type { Diagnostic, Limits } from "../codec.js";
/** One shared golden set; this annotation keeps JSON compiler options out of consumer declarations. */
interface FixtureSet {
  valid: WireRecord[];
  invalid: Array<{
    name: string;
    raw: string;
    code: Diagnostic;
    limits?: Partial<Limits>;
  }>;
  commandHash: string;
  a2uiServer: unknown[];
}
export const fixtures: FixtureSet = data as unknown as FixtureSet;
