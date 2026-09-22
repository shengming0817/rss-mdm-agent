import { spawnSync } from "node:child_process";
import { readFileSync, writeFileSync } from "node:fs";
import { compile } from "json-schema-to-typescript";
import { format } from "prettier";
const result = spawnSync(
  "cargo",
  ["run", "--quiet", "-p", "native-process", "--example", "schema"],
  { encoding: "utf8" },
);
if (result.status !== 0) throw new Error(result.stderr);
const schema = JSON.parse(result.stdout);
const path = "packages/ai-host/src/process-contract.ts";
const text = await format(
  await compile(schema, "Ready", {
    bannerComment: "// @generated from native-process::Ready. Do not edit.",
  }),
  { parser: "typescript" },
);
if (process.argv.includes("--check")) {
  if (readFileSync(path, "utf8") !== text)
    throw new Error("process contract drift");
} else writeFileSync(path, text);

const service = spawnSync(
  "cargo",
  ["run", "--quiet", "-p", "local-service", "--example", "schema"],
  { encoding: "utf8" },
);
if (service.status !== 0) throw new Error(service.stderr);
const servicePath = "apps/desktop/src/settings/service-contract.ts";
const serviceText = await format(
  await compile(JSON.parse(service.stdout), "ServiceView", {
    bannerComment:
      "// @generated from local-service::ServiceView. Do not edit.",
  }),
  { parser: "typescript" },
);
if (process.argv.includes("--check")) {
  if (readFileSync(servicePath, "utf8") !== serviceText)
    throw new Error("service contract drift");
} else writeFileSync(servicePath, serviceText);
