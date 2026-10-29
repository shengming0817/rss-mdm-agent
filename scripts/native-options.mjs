import { isAbsolute } from "node:path";

/** Parse before any platform side effects; acceptance modes must be explicit. */
export function nativeOptions(args) {
  const flags = new Set();
  const values = new Map();
  for (let at = 0; at < args.length; at++) {
    const flag = args[at];
    if (flags.has(flag) || values.has(flag))
      throw Error(`duplicate native option: ${flag}`);
    if (
      ["--visual", "--controlled-service", "--security", "--baseline"].includes(
        flag,
      )
    )
      flags.add(flag);
    else if (["--candidate", "--authorization-password-file"].includes(flag)) {
      const value = args[++at];
      if (!value || value.startsWith("--"))
        throw Error(`missing native option value: ${flag}`);
      values.set(flag, value);
    } else throw Error(`unknown native option: ${flag}`);
  }
  const visualOnly = flags.has("--visual"),
    controlledService = flags.has("--controlled-service");
  if (visualOnly === controlledService)
    throw Error(
      "choose exactly one native mode: --visual or --controlled-service",
    );
  const security = flags.has("--security"),
    baseline = flags.has("--baseline");
  const candidatePath = values.get("--candidate"),
    authorizationPasswordFile = values.get("--authorization-password-file");
  if (
    (security || candidatePath || authorizationPasswordFile) &&
    !controlledService
  )
    throw Error(
      "security, candidate and authorization password require controlled-service mode",
    );
  if (baseline && !visualOnly) throw Error("baseline requires visual mode");
  if (authorizationPasswordFile && !isAbsolute(authorizationPasswordFile))
    throw Error("authorization password file requires an absolute path");
  return {
    visualOnly,
    controlledService,
    security,
    baseline,
    candidatePath,
    authorizationPasswordFile,
  };
}
