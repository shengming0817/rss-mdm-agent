import { createHash } from "node:crypto";
import { readFileSync } from "node:fs";
import { Ajv2020 } from "ajv/dist/2020.js";
import canonicalize from "canonicalize";
import { visit } from "jsonc-parser";
import type { WireRecord, Command, Event, Id } from "./wire.js";

/** Mandatory host bounds. Counts cover the entire envelope, including dynamic JSON. */
export interface Limits {
  maxBytes: number;
  maxTextBytes: number;
  maxDepth: number;
  maxNodes: number;
}
export type Diagnostic =
  | "configuration"
  | "encoding"
  | "duplicate_key"
  | "version"
  | "schema"
  | "limit"
  | "number"
  | "context";
/** Value-free error. Never retains an input value or provider exception as cause. */
export class ContractError extends Error {
  constructor(readonly code: Diagnostic) {
    super(`AI contract: ${code}`);
    this.name = "ContractError";
  }
}
const schema = JSON.parse(
  readFileSync(
    new URL("../schema/runtime.schema.json", import.meta.url),
    "utf8",
  ),
);
const validator = new Ajv2020({
  strict: true,
  allErrors: false,
  validateFormats: false,
});
const valid = validator.compile<WireRecord>(schema);
const checkId = validator.compile<Id>(schema.$defs.Id);
/** Validate the schema-owned primitive, including its type and length bounds. */
export function isId(value: unknown): value is Id {
  return checkId(value);
}
const byteLength = (s: string) => Buffer.byteLength(s, "utf8");
const unicode = (s: string) => {
  for (const c of s) {
    const n = c.codePointAt(0)!;
    if (n >= 0xd800 && n <= 0xdfff) return false;
  }
  return true;
};
function checkLimits(limits: Limits): void {
  for (const k of ["maxBytes", "maxTextBytes", "maxDepth", "maxNodes"] as const)
    if (!Number.isSafeInteger(limits[k]) || limits[k] < 1)
      throw new ContractError("configuration");
  if (limits.maxDepth > 64) throw new ContractError("configuration");
}
/** Decode strict bounded V2 JSON. Schema validity does not establish authority. */
export function decode(input: string | Uint8Array, limits: Limits): WireRecord {
  checkLimits(limits);
  if (
    typeof input === "string"
      ? byteLength(input) > limits.maxBytes
      : input.byteLength > limits.maxBytes
  )
    throw new ContractError("limit");
  let source: string;
  try {
    source =
      typeof input === "string"
        ? input
        : new TextDecoder("utf-8", { fatal: true }).decode(input);
  } catch {
    throw new ContractError("encoding");
  }
  if (!unicode(source)) throw new ContractError("encoding");
  const stack: Array<Set<string> | null> = [];
  let nodes = 0,
    textBytes = 0;
  const count = () => {
    if (++nodes > limits.maxNodes) throw new ContractError("limit");
  };
  const text = (value: string) => {
    if (!unicode(value)) throw new ContractError("encoding");
    textBytes += byteLength(value);
    if (textBytes > limits.maxTextBytes) throw new ContractError("limit");
  };
  const begin = (keys: Set<string> | null) => {
    count();
    if (stack.length + 1 > limits.maxDepth) throw new ContractError("limit");
    stack.push(keys);
  };
  visit(
    source,
    {
      onObjectBegin: () => begin(new Set()),
      onArrayBegin: () => begin(null),
      onObjectEnd: () => {
        stack.pop();
      },
      onArrayEnd: () => {
        stack.pop();
      },
      onObjectProperty: (key) => {
        const keys = stack.at(-1);
        if (!keys || keys.has(key)) throw new ContractError("duplicate_key");
        keys.add(key);
        text(key);
      },
      onLiteralValue: (value) => {
        count();
        if (typeof value === "string") text(value);
        if (
          typeof value === "number" &&
          (!Number.isFinite(value) ||
            (Number.isInteger(value) && !Number.isSafeInteger(value)))
        )
          throw new ContractError("number");
      },
      onError: () => {
        throw new ContractError("encoding");
      },
    },
    {
      disallowComments: true,
      allowTrailingComma: false,
      allowEmptyContent: false,
    },
  );
  let value: unknown;
  try {
    value = JSON.parse(source);
  } catch {
    throw new ContractError("encoding");
  }
  if (
    value &&
    typeof value === "object" &&
    "schemaVersion" in value &&
    value.schemaVersion !== 2
  )
    throw new ContractError("version");
  if (!valid(value)) throw new ContractError("schema");
  checkContext(value);
  return value;
}
function checkContext(value: WireRecord): void {
  if (
    value.kind === "command" &&
    value.input.type === "prompt" &&
    (value.input.policy === "steer") !== !!value.input.targetRunId
  )
    throw new ContractError("context");
  if (
    value.kind === "receipt" &&
    (value.acceptedAtMs > value.retryUntilMs ||
      value.retryUntilMs > value.receiptUntilMs)
  )
    throw new ContractError("context");
  if (value.kind === "commandRecord") {
    if (
      value.command.commandId !== value.receipt.commandId ||
      value.command.sessionId !== value.receipt.namespace.sessionId ||
      hash(value.command) !== value.receipt.contentHash
    )
      throw new ContractError("context");
    if (
      (value.state === "terminal") !== !!value.outcome ||
      ((value.state === "dispatching" ||
        value.state === "running" ||
        value.state === "reconciliation_required") &&
        !value.dispatch)
    )
      throw new ContractError("context");
    checkContext(value.command);
    checkContext(value.receipt);
  }
  if (
    value.kind === "interaction" &&
    (value.status === "answered") !== !!value.responseCommandId
  )
    throw new ContractError("context");
}
function hash(command: Command): string {
  return createHash("sha256").update(canonicalize(command)!).digest("hex");
}
/** Serialize plain constructed JSON under the same budgets, without invoking accessors/toJSON. */
export function boundedJson(value: unknown, limits: Limits): string {
  checkLimits(limits);
  let nodes = 0,
    textBytes = 0,
    bytes = 0;
  const parts: string[] = [],
    active = new Set<object>();
  const emit = (part: string) => {
    bytes += byteLength(part);
    if (bytes > limits.maxBytes) throw new ContractError("limit");
    parts.push(part);
  };
  const text = (v: string) => {
    if (!unicode(v)) throw new ContractError("encoding");
    textBytes += byteLength(v);
    if (textBytes > limits.maxTextBytes || byteLength(v) > limits.maxBytes)
      throw new ContractError("limit");
    emit(JSON.stringify(v));
  };
  const walk = (v: unknown, depth: number): void => {
    if (++nodes > limits.maxNodes) throw new ContractError("limit");
    if (typeof v === "string") {
      text(v);
      return;
    }
    if (typeof v === "number") {
      if (
        !Number.isFinite(v) ||
        (Number.isInteger(v) && !Number.isSafeInteger(v))
      )
        throw new ContractError("number");
      emit(JSON.stringify(v));
      return;
    }
    if (v === null || typeof v === "boolean") {
      emit(JSON.stringify(v));
      return;
    }
    if (
      typeof v !== "object" ||
      (!Array.isArray(v) &&
        Object.getPrototypeOf(v) !== Object.prototype &&
        Object.getPrototypeOf(v) !== null)
    )
      throw new ContractError("encoding");
    if (depth + 1 > limits.maxDepth) throw new ContractError("limit");
    if (active.has(v)) throw new ContractError("encoding");
    active.add(v);
    const array = Array.isArray(v),
      keys = Object.keys(v);
    if (
      keys.length > limits.maxNodes - nodes ||
      (array && v.length !== keys.length)
    )
      throw new ContractError("limit");
    emit(array ? "[" : "{");
    for (const [i, key] of keys.entries()) {
      if (i) emit(",");
      if (array) {
        if (key !== String(i)) throw new ContractError("encoding");
      } else {
        text(key);
        emit(":");
      }
      const property = Object.getOwnPropertyDescriptor(v, key);
      if (!property || !("value" in property))
        throw new ContractError("encoding");
      walk(property.value, depth + 1);
    }
    emit(array ? "]" : "}");
    active.delete(v);
  };
  try {
    walk(value, 0);
    return parts.join("");
  } catch (error) {
    if (error instanceof ContractError) throw error;
    throw new ContractError("encoding");
  }
}
/** Validate programmatic data before producing its JCS/SHA-256 identity. */
export function fingerprint(command: Command, limits: Limits): string {
  const checked = decode(boundedJson(command, limits), limits);
  if (checked.kind !== "command") throw new ContractError("schema");
  return hash(checked);
}
/** Delivery identity is SHA-256(JCS({event, target})); operationId is the storage key. */
export function deliveryFingerprint(
  event: Event,
  target: string,
  limits: Limits,
): string {
  const checked = decode(boundedJson(event, limits), limits);
  if (checked.kind !== "event" || !isId(target))
    throw new ContractError("schema");
  const payload = JSON.parse(boundedJson({ event: checked, target }, limits));
  return createHash("sha256").update(canonicalize(payload)!).digest("hex");
}
