import { helperOutput } from "./preparation.mjs";
import { readFileSync, writeFileSync } from "node:fs";
import { compile } from "json-schema-to-typescript";
import { format } from "prettier";
const result = JSON.parse(await helperOutput("processSchema"));
const schema = result;
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

const serviceSchema = JSON.parse(await helperOutput("serviceSchema"));
const registrationSchema = serviceSchema["x-rss-registration-schema"];
delete serviceSchema["x-rss-registration-schema"];
const versionPath = "packages/execution-bindings/src/service-version.mjs";
const versionText = `// @generated from execution-ipc::host::IPC_VERSION. Do not edit.\nexport const IPC_VERSION = ${serviceSchema["x-rss-ipc-version"]};\nexport const DEPLOYMENT_VERSION = ${serviceSchema["x-rss-deployment-version"]};\n`;
if (process.argv.includes("--check")) {
  if (readFileSync(versionPath, "utf8") !== versionText)
    throw new Error("service version drift");
} else writeFileSync(versionPath, versionText);

const servicePath = "apps/desktop/src/settings/service-contract.ts";
const serviceText = await format(
  (await compile(serviceSchema, "ServiceView", {
    bannerComment:
      "// @generated from execution-ipc::host::{ServiceView, RegistrationStatus}. Do not edit.",
  })) +
    "\n" +
    (await compile(registrationSchema, "RegistrationStatus", {
      bannerComment: "",
    })),
  { parser: "typescript" },
);
if (process.argv.includes("--check")) {
  if (readFileSync(servicePath, "utf8") !== serviceText)
    throw new Error("service contract drift");
} else writeFileSync(servicePath, serviceText);

const fixtureResult = JSON.parse(await helperOutput("serviceFixtures"));
const fixtureText = await format(JSON.stringify(fixtureResult), {
  parser: "json",
});
const fixturePath = "tests/assistant/service-fixtures.json";
if (process.argv.includes("--check")) {
  if (readFileSync(fixturePath, "utf8") !== fixtureText)
    throw new Error("service fixture drift");
} else writeFileSync(fixturePath, fixtureText);
