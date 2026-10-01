// ref: Node.js lib/util.js@v24.14.1 (parseEnv without mutating process.env).
import { readFileSync } from "node:fs";
import { join } from "node:path";
import { parseEnv } from "node:util";

export const organizationBuildInput = "RSS_BUILD_MDM_ORGANIZATION";
const fields = {
  origin: "RSS_MDM_ORIGIN",
  tenant: "RSS_MDM_TENANT_ID",
  label: "RSS_MDM_ORGANIZATION_LABEL",
};

export function desktopBuildEnvironment(root, inherited = process.env) {
  let values;
  try {
    values = parseEnv(readFileSync(join(root, ".env"), "utf8"));
  } catch {
    throw new Error(
      "Cannot read root .env; copy env.example to .env and configure the backend connection",
    );
  }
  const organization = {};
  for (const [field, name] of Object.entries(fields)) {
    if (!values[name]?.trim())
      throw new Error(
        `Configure ${name} in root .env before starting or packaging the desktop`,
      );
    organization[field] = values[name];
  }
  const env = { ...inherited };
  delete env[organizationBuildInput];
  for (const name of Object.values(fields)) delete env[name];
  env[organizationBuildInput] = JSON.stringify(organization);
  return env;
}
