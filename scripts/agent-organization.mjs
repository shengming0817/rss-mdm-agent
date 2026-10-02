// Generate new installation inputs from the same root .env used by desktop builds.
import { readFileSync, writeFileSync, unlinkSync, existsSync } from "node:fs";
import { isAbsolute, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { organizationConfiguration } from "./desktop-organization.mjs";

export function agentOrganizationConfiguration(root, template, caOutput) {
  const organization = organizationConfiguration(root);
  if (template.version !== 2 || template.ipc_version !== 6)
    throw new Error(
      "Agent template must use current deployment and IPC versions",
    );
  const origin = new URL(organization.origin);
  if (
    origin.protocol !== "https:" ||
    !origin.hostname ||
    origin.username ||
    origin.password ||
    origin.search ||
    origin.hash ||
    origin.pathname !== "/" ||
    origin.hostname === "example" ||
    origin.hostname.endsWith(".example")
  )
    throw new Error("Configure a real HTTPS RSS_MDM_ORIGIN");
  if (
    !/^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i.test(
      organization.tenant,
    ) ||
    /^0{8}-0{4}-0{4}-0{4}-0{12}$/.test(organization.tenant)
  )
    throw new Error("Configure a non-zero UUID RSS_MDM_TENANT_ID");
  if (organization.ca_pem && (!caOutput || !isAbsolute(caOutput)))
    throw new Error("--ca-output must be the absolute installed CA path");
  return {
    config: {
      ...template,
      origin: origin.origin,
      tenant: organization.tenant.toLowerCase(),
      ca_file: organization.ca_pem ? caOutput : null,
    },
    ca: organization.ca_pem,
  };
}

if (
  process.argv[1] &&
  resolve(process.argv[1]) === fileURLToPath(import.meta.url)
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
    }
    writeFileSync(options["--output"], JSON.stringify(config, null, 2) + "\n", {
      flag: "wx",
      mode: 0o600,
    });
  } catch (error) {
    if (createdCa) unlinkSync(options["--ca-output"]);
    throw error;
  }
  console.log(
    "Agent organization configuration generated; install under administrator protection before registration.",
  );
}
