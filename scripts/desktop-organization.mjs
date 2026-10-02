// ref: Node.js lib/util.js@v24.14.1 (parseEnv without mutating process.env).
import { readFileSync } from "node:fs";
import { join, isAbsolute } from "node:path";
import { X509Certificate } from "node:crypto";
import { parseEnv } from "node:util";

export const organizationBuildInput = "RSS_BUILD_MDM_ORGANIZATION";
const fields = {
  origin: "RSS_MDM_ORIGIN",
  tenant: "RSS_MDM_TENANT_ID",
  label: "RSS_MDM_ORGANIZATION_LABEL",
};

export function organizationConfiguration(root) {
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
  const caFile = values.RSS_MDM_CA_FILE?.trim();
  if (caFile) {
    try {
      if (!isAbsolute(caFile)) throw new Error();
      const pem = readFileSync(caFile, "utf8");
      if (
        Buffer.byteLength(pem) > 65536 ||
        !/^\s*-----BEGIN CERTIFICATE-----[A-Za-z0-9+/=\s]+-----END CERTIFICATE-----\s*$/.test(
          pem,
        )
      )
        throw new Error();
      const certificate = new X509Certificate(pem);
      if (!certificate.ca) throw new Error();
      organization.ca_pem = certificate.toString();
    } catch {
      throw new Error(
        "RSS_MDM_CA_FILE must be an absolute path to one PEM CA public certificate (no private key)",
      );
    }
  }
  return organization;
}

export function desktopBuildEnvironment(root, inherited = process.env) {
  const organization = organizationConfiguration(root);
  const env = { ...inherited };
  delete env[organizationBuildInput];
  delete env.RSS_MDM_CA_FILE;
  for (const name of Object.values(fields)) delete env[name];
  env[organizationBuildInput] = JSON.stringify(organization);
  return env;
}
