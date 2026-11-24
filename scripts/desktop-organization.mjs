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

export function publicCa(pem) {
  if (
    Buffer.byteLength(pem) > 65536 ||
    !/^\s*-----BEGIN CERTIFICATE-----[A-Za-z0-9+/=\s]+-----END CERTIFICATE-----\s*$/.test(
      pem,
    )
  )
    throw new Error();
  const certificate = new X509Certificate(pem);
  if (!certificate.ca) throw new Error();
  return certificate.toString();
}

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
      organization.ca_pem = publicCa(pem);
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

export function employeePortalConfiguration(
  organization,
  desktopConfiguration,
) {
  const defaults = organization.modules;
  const mdmEnabled = defaults.mdm?.state === "enabled";
  const daEnabled = defaults.da?.state === "enabled";
  const product = organization.employeeIssuer ?? (mdmEnabled ? "mdm" : "da");
  if (
    !["mdm", "da"].includes(product) ||
    (organization.employeeIssuer !== undefined &&
      defaults[product]?.state !== "enabled")
  )
    throw Error("Employee issuer must select an enabled DA or MDM module");
  if (
    mdmEnabled &&
    daEnabled &&
    (organization.employeeApplication || organization.recoveryReturnTarget) &&
    !organization.employeeIssuer
  )
    throw Error("Select the employee issuer for the coinstalled modules");
  const owner = defaults[product]?.backend;
  const daBrowser = product === "da" ? owner?.browser : null;
  const issuer = daBrowser
    ? { ...daBrowser, tenant: owner.tenant }
    : desktopConfiguration;
  const recoveryReturnTarget = organization.recoveryReturnTarget;
  if (
    recoveryReturnTarget !== undefined &&
    (!issuer ||
      typeof recoveryReturnTarget !== "string" ||
      !/^[a-zA-Z0-9_.-]{1,128}$/.test(recoveryReturnTarget))
  ) {
    throw Error(
      "Employee recovery requires the registered same-origin OIDC return target ID",
    );
  }
  let application = organization.employeeApplication;
  if (application !== undefined) {
    if (
      !issuer ||
      !application ||
      typeof application !== "object" ||
      Object.keys(application).some(
        (k) => !["relyingParty", "launchUrl"].includes(k),
      ) ||
      typeof application.relyingParty !== "string" ||
      !/^[a-zA-Z0-9_-]{1,64}$/.test(application.relyingParty) ||
      typeof application.launchUrl !== "string" ||
      application.launchUrl.length > 2048
    )
      throw Error(
        "Employee application requires the deployed RP ID and its fixed HTTPS launch URL",
      );
    const url = new URL(application.launchUrl);
    if (
      url.protocol !== "https:" ||
      url.username ||
      url.password ||
      url.href.includes("?") ||
      url.href.includes("#") ||
      url.href.length > 2048
    )
      throw Error("Invalid employee application launch URL");
    application = { ...application, launchUrl: url.href };
  }
  if (!issuer) return null;
  if (!owner || issuer.tenant?.toLowerCase() !== owner.tenant)
    throw Error(
      "Employee browser issuer must belong to the selected module tenant",
    );
  const url = new URL(issuer.origin);
  const host = url.hostname.replace(/\.+$/, "");
  if (
    url.protocol !== "https:" ||
    !host ||
    url.username ||
    url.password ||
    url.pathname !== "/" ||
    url.href.includes("?") ||
    url.href.includes("#")
  )
    throw Error("Employee issuer requires a public HTTPS origin");
  url.hostname = host;
  return {
    product,
    instance: owner.instance,
    tenant: owner.tenant,
    origin: url.origin,
    ca_pem: issuer.ca_pem == null ? undefined : publicCa(issuer.ca_pem),
    application,
    recovery_return_target: recoveryReturnTarget,
  };
}
