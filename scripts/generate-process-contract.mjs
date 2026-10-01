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
const privateLink = JSON.parse(
  readFileSync("crates/native-process/private-link-v1.json", "utf8"),
);
const path = "packages/ai-host/src/process-contract.ts";
const text = await format(
  `${await compile(schema, "Ready", {
    bannerComment:
      "// @generated from native-process::Ready and private-link-v1.json. Do not edit.",
  })}\nexport const privateLinkV1 = ${JSON.stringify(privateLink)} as const;\n`,
  { parser: "typescript" },
);
if (process.argv.includes("--check")) {
  if (readFileSync(path, "utf8") !== text)
    throw new Error("process contract drift");
} else writeFileSync(path, text);

const service = spawnSync(
  "cargo",
  ["run", "--quiet", "-p", "execution-runner", "--example", "service-schema"],
  { encoding: "utf8" },
);
if (service.status !== 0) throw new Error(service.stderr);
const servicePath = "apps/desktop/src/settings/service-contract.ts";
const serviceText = await format(
  await compile(JSON.parse(service.stdout), "ServiceView", {
    bannerComment:
      "// @generated from execution-runner::host::ServiceView. Do not edit.",
  }),
  { parser: "typescript" },
);
if (process.argv.includes("--check")) {
  if (readFileSync(servicePath, "utf8") !== serviceText)
    throw new Error("service contract drift");
} else writeFileSync(servicePath, serviceText);

const fixtureResult = spawnSync(
  "cargo",
  [
    "run",
    "--quiet",
    "--locked",
    "-p",
    "execution-runner",
    "--example",
    "service-fixtures",
  ],
  { encoding: "utf8" },
);
if (fixtureResult.status !== 0) throw new Error(fixtureResult.stderr);
const fixtureText = await format(
  JSON.stringify(JSON.parse(fixtureResult.stdout)),
  { parser: "json" },
);
const fixturePath = "tests/assistant/service-fixtures.json";
if (process.argv.includes("--check")) {
  if (readFileSync(fixturePath, "utf8") !== fixtureText)
    throw new Error("service fixture drift");
} else writeFileSync(fixturePath, fixtureText);
