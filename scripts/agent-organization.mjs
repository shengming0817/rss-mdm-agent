// Generate new installation inputs from the same root .env used by desktop builds.
import {
  readFileSync,
  writeFileSync,
  unlinkSync,
  existsSync,
  chmodSync,
  realpathSync,
  statSync,
} from "node:fs";
import { join, isAbsolute } from "node:path";
import { fileURLToPath } from "node:url";
import {
  IPC_VERSION,
  DEPLOYMENT_VERSION,
} from "../packages/execution-bindings/src/service-version.mjs";
import { organizationConfiguration } from "./desktop-organization.mjs";
import { parseEnv } from "node:util";
import { X509Certificate } from "node:crypto";

export function agentOrganizationConfiguration(root, template, caOutput) {
  const organization = organizationConfiguration(root);
  const values = parseEnv(readFileSync(join(root, ".env"), "utf8"));
  if (
    template.version !== DEPLOYMENT_VERSION ||
    template.ipc_version !== IPC_VERSION
  )
    throw new Error(
      "Agent template must use current deployment and IPC versions",
    );
  if (values.RSS_AGENT_ORIGIN?.trim())
    throw new Error(
      "RSS_AGENT_ORIGIN is retired; use a signed RSS_AGENT_CONNECTIONS_FILE",
    );
  if ("origin" in template || "task_trust_origins" in template)
    throw new Error("Retired Agent deployment fields");
  let connections;
  try {
    const path = values.RSS_AGENT_CONNECTIONS_FILE;
    if (!path || !isAbsolute(path) || statSync(path).size > 65536)
      throw new Error();
    connections = JSON.parse(readFileSync(path, "utf8"));
  } catch {
    throw new Error(
      "RSS_AGENT_CONNECTIONS_FILE must name a bounded signed connection file",
    );
  }
  const publicCa = (name, required = false) => {
    const source = values[name]?.trim();
    if (!source && !required) return undefined;
    try {
      if (!source || !isAbsolute(source)) throw new Error();
      const pem = readFileSync(source, "utf8");
      if (
        Buffer.byteLength(pem) > 32768 ||
        !/^\s*-----BEGIN CERTIFICATE-----[A-Za-z0-9+/=\s]+-----END CERTIFICATE-----\s*$/.test(
          pem,
        )
      )
        throw new Error();
      const certificate = new X509Certificate(pem);
      if (!certificate.ca) throw new Error();
      return certificate.toString();
    } catch {
      throw new Error(
        `${name} must be an absolute path to one PEM CA public certificate`,
      );
    }
  };
  const ca = publicCa("RSS_AGENT_CA_FILE", true);
  if (
    !/^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i.test(
      organization.tenant,
    ) ||
    /^0{8}-0{4}-0{4}-0{4}-0{12}$/.test(organization.tenant)
  )
    throw new Error("Configure a non-zero UUID RSS_MDM_TENANT_ID");
  if (ca && (!caOutput || !isAbsolute(caOutput)))
    throw new Error("--ca-output must be the absolute installed CA path");
  return {
    config: {
      ...template,
      connections,
      tenant: organization.tenant.toLowerCase(),
      ca_file: ca ? caOutput : null,
    },
    ca,
  };
}

if (
  process.argv[1] &&
  realpathSync(process.argv[1]) === fileURLToPath(import.meta.url)
) {
  const args = process.argv.slice(2);
  if (
    args.length % 2 ||
    args.some(
      (arg, index) =>
        index % 2 === 0 &&
        !["--config", "--output", "--ca-output"].includes(arg),
    )
  )
    throw new Error(
      "Usage: node scripts/agent-organization.mjs --config TEMPLATE --output NEW_CONFIG [--ca-output ABSOLUTE_INSTALLED_CA]",
    );
  const options = Object.fromEntries(
    Array.from({ length: args.length / 2 }, (_, index) =>
      args.slice(index * 2, index * 2 + 2),
    ),
  );
  if (!options["--config"] || !options["--output"])
    throw new Error("--config and --output are required");
  const root = fileURLToPath(new URL("../", import.meta.url));
  const template = JSON.parse(readFileSync(options["--config"], "utf8"));
  const { config, ca } = agentOrganizationConfiguration(
    root,
    template,
    options["--ca-output"],
  );
  if (
    existsSync(options["--output"]) ||
    (ca && existsSync(options["--ca-output"]))
  )
    throw new Error(
      "Output files must be new; do not overwrite an existing installation",
    );
  let createdCa = false;
  try {
    if (ca) {
      writeFileSync(options["--ca-output"], ca, { flag: "wx", mode: 0o644 });
      createdCa = true;
      chmodSync(options["--ca-output"], 0o644);
    }
    writeFileSync(options["--output"], JSON.stringify(config, null, 2) + "\n", {
      flag: "wx",
      mode: 0o600,
    });
    chmodSync(options["--output"], 0o644);
  } catch (error) {
    if (createdCa) unlinkSync(options["--ca-output"]);
    throw error;
  }
  console.log(
    "Agent organization configuration generated; install under administrator protection before registration.",
  );
}
