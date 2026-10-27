// ref: json-schema-to-typescript src/typesOfSchema.ts / src/parser.ts
// @7f72770eb854328c96b112be445da0306bebdbaf (16.0.0): tsType preserves
// annotation-only schemas as unknown instead of inferring an object or default's type.
export function typescriptSchema(schema) {
  const annotations = new Set([
    "title",
    "description",
    "$comment",
    "default",
    "examples",
    "readOnly",
    "writeOnly",
    "deprecated",
    "x-rss-extension",
  ]);
  const maps = new Set([
    "$defs",
    "definitions",
    "properties",
    "patternProperties",
    "dependentSchemas",
  ]);
  const children = new Set([
    "allOf",
    "anyOf",
    "oneOf",
    "prefixItems",
    "items",
    "additionalItems",
    "additionalProperties",
    "propertyNames",
    "contains",
    "not",
    "if",
    "then",
    "else",
    "unevaluatedItems",
    "unevaluatedProperties",
    "contentSchema",
  ]);
  const projected = structuredClone(schema);
  function visit(node) {
    if (Array.isArray(node)) return node.forEach(visit);
    if (!node || typeof node !== "object") return;
    const keys = Object.keys(node);
    if (keys.length && keys.every((key) => annotations.has(key)))
      node.tsType = "unknown";
    // Preserve free-form object metadata before the TS compiler applies its
    // closed-object default. Structured command/DTO projections keep that default.
    // ref: json-schema-to-typescript src/normalizer.ts (Default additionalProperties).
    if (
      (node.type === "object" ||
        (Array.isArray(node.type) && node.type.includes("object"))) &&
      !("additionalProperties" in node) &&
      !("properties" in node) &&
      !("patternProperties" in node)
    )
      node.additionalProperties = true;
    for (const [key, value] of Object.entries(node)) {
      if (maps.has(key)) Object.values(value).forEach(visit);
      else if (children.has(key)) visit(value);
    }
  }
  visit(projected);
  return projected;
}
