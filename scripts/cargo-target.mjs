import { resolve } from "node:path";

export function cargoTargetDir(root, env = process.env) {
  const explicit = env.CARGO_TARGET_DIR;
  const configured = env.CARGO_BUILD_TARGET_DIR;
  for (const value of [explicit, configured])
    if (value !== undefined && !value.trim())
      throw Error("Cargo target directory must not be empty");
  if (
    explicit !== undefined &&
    configured !== undefined &&
    resolve(root, explicit) !== resolve(root, configured)
  )
    throw Error("conflicting Cargo target directories");
  return resolve(root, explicit ?? configured ?? "target");
}
