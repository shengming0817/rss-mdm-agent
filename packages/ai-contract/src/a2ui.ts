import validateMessage from "./validate-surface.js";
import { boundedJson, decode, type Limits } from "./codec.js";
import type { SurfaceState } from "./wire.js";

/** Product catalog is an explicitly restricted subset of the fixed upstream catalog. */
export const interactionCatalog = {
  version: "v0.9.1",
  catalogId: "urn:rss-mdm-agent:a2ui:interaction",
  catalogVersion: "1",
} as const;
export const accessLimits: Limits = {
  maxBytes: 262144,
  maxTextBytes: 131072,
  maxDepth: 32,
  maxNodes: 16384,
};
export class SurfaceError extends Error {
  constructor(readonly code: "schema" | "catalog" | "component" | "lifecycle") {
    super(`A2UI ${code} rejected`);
  }
}
/** Validate the complete recovery batch before publishing or touching the renderer.
 * No functions, markup renderer, remote resources, themes or data-model exfiltration. */
export function validateSurface(
  input: SurfaceState,
  limits: Limits = accessLimits,
): SurfaceState {
  const record = decode(boundedJson(input, limits), limits);
  if (record.kind !== "surface") throw new SurfaceError("schema");
  const surface = record;
  if (
    surface.catalogId !== interactionCatalog.catalogId ||
    surface.catalogVersion !== interactionCatalog.catalogVersion
  )
    throw new SurfaceError("catalog");
  let active = false,
    created = false;
  const components = new Map<string, Record<string, unknown>>();
  const safe = (value: unknown): void => {
    if (!value || typeof value !== "object") return;
    if (Array.isArray(value)) {
      value.forEach(safe);
      return;
    }
    for (const [key, child] of Object.entries(value)) {
      if (
        [
          "call",
          "functionCall",
          "__proto__",
          "prototype",
          "constructor",
        ].includes(key)
      )
        throw new SurfaceError("component");
      if (
        key === "path" &&
        typeof child === "string" &&
        (!child.startsWith("/") ||
          child
            .split("/")
            .some((p) => ["__proto__", "constructor", "prototype"].includes(p)))
      )
        throw new SurfaceError("component");
      safe(child);
    }
  };
  for (const message of surface.messages) {
    if (
      !validateMessage(message) ||
      message.version !== interactionCatalog.version
    )
      throw new SurfaceError("schema");
    safe(message);
    const operation = [
      "createSurface",
      "updateComponents",
      "updateDataModel",
      "deleteSurface",
    ].find((k) => k in message)!;
    const body = message[operation] as Record<string, unknown>;
    if (body.surfaceId !== surface.surfaceId)
      throw new SurfaceError("lifecycle");
    if (operation === "createSurface") {
      if (
        created ||
        body.catalogId !== surface.catalogId ||
        "theme" in body ||
        body.sendDataModel === true
      )
        throw new SurfaceError("catalog");
      created = active = true;
    } else {
      if (!active) throw new SurfaceError("lifecycle");
      if (operation === "deleteSurface") active = false;
      if (operation === "updateComponents") {
        const ids = new Set<string>();
        for (const component of body.components as Record<string, unknown>[]) {
          if (
            !["Text", "Column", "Button", "TextField"].includes(
              component.component as string,
            )
          )
            throw new SurfaceError("component");
          const id = component.id as string;
          if (ids.has(id)) throw new SurfaceError("component");
          ids.add(id);
          // Dynamic child templates and validation/function calls are outside this catalog.
          if (
            (component.children && !Array.isArray(component.children)) ||
            "checks" in component
          )
            throw new SurfaceError("component");
          components.set(id, component);
        }
      }
    }
  }
  if (!created || active !== (surface.status === "active"))
    throw new SurfaceError("lifecycle");
  const visit = (id: string, ancestors: Set<string>): void => {
    const node = components.get(id);
    if (!node || ancestors.has(id) || ancestors.size > 24)
      throw new SurfaceError("component");
    const path = new Set(ancestors).add(id);
    if (node.child) visit(node.child as string, path);
    for (const child of (node.children ?? []) as string[]) visit(child, path);
  };
  visit("root", new Set());
  const source = components.get(surface.sourceComponentId);
  const action = source?.action as { event?: { name?: string } } | undefined;
  if (
    source?.component !== "Button" ||
    action?.event?.name !== surface.eventName
  )
    throw new SurfaceError("component");
  return surface;
}
