import { resolve } from "node:path";

export const cargoTargetDir = (root) =>
  resolve(root, process.env.CARGO_TARGET_DIR ?? "target");
