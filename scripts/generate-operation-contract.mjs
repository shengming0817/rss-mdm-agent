import { readFileSync } from "node:fs";
import { compile } from "json-schema-to-typescript";
import { format } from "prettier";

// The product schema is the sole source. Upstream component contracts stay upstream.
export async function generateOperations(outputs, compileValidator) {
  const read = (path) =>
    JSON.parse(readFileSync(new URL(path, import.meta.url), "utf8"));
  const schema = read("../packages/ai-contract/schema/operation.schema.json");
  const common =
    "https://a2ui.org/specification/v0_9/common_types.json#/$defs/";
  const basic = read(
    "../packages/ai-contract/schema/upstream/a2ui/catalog.json",
  );
  const components = Object.fromEntries(
    ["Text", "Column", "Button", "TextField"].map((id) => [
      id,
      basic.components[id],
    ]),
  );
  for (const id of schema["x-components"]) {
    if (components[id]) continue;
    components[id] = {
      type: "object",
      properties: {
        id: { type: "string" },
        component: { const: id },
        label: { type: "string" },
        fieldId: { type: "string" },
        children: { $ref: common + "ChildList" },
        value: { $ref: common + "DynamicValue" },
      },
      required: ["id", "component"],
      additionalProperties: false,
    };
  }
  const catalog = {
    catalogId: "urn:rss-mdm-agent:a2ui:operation:1",
    components,
    functions: {},
    $defs: { CatalogComponentCommon: basic.$defs.CatalogComponentCommon },
  };
  outputs.set(
    "packages/ai-contract/src/operation-catalog.ts",
    await format(
      "// @generated from operation.schema.json and fixed upstream catalog.\nexport const operationCatalog = " +
        JSON.stringify(catalog) +
        ";\n",
      { parser: "typescript" },
    ),
  );
  outputs.set(
    "packages/ai-contract/src/operation-types.ts",
    await format(
      await compile(schema, "OperationContract", {
        ignoreMinAndMaxItems: true,
        additionalProperties: false,
        unreachableDefinitions: true,
        format: false,
        bannerComment:
          "// @generated from schema/operation.schema.json. Do not edit.",
      }),
      { parser: "typescript" },
    ),
  );
  for (const [name, definition] of [
    ["presentation", "OperationPresentation"],
    ["snapshot", "OperationSnapshot"],
    ["event", "OperationEvent"],
  ]) {
    outputs.set(
      `packages/ai-contract/src/validate-operation-${name}.ts`,
      await format(
        compileValidator(
          {
            $schema: schema.$schema,
            $defs: schema.$defs,
            $ref: `#/$defs/${definition}`,
          },
          true,
        ),
        { parser: "typescript" },
      ),
    );
  }
}
