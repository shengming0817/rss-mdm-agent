// ref: Vite v6.4.3 plugins/manifest.ts; Rollup v4.59.0 Chunk.ts (rendered module inventory).
import { createHash } from "node:crypto";
import {
  existsSync,
  lstatSync,
  readFileSync,
  readdirSync,
  writeFileSync,
} from "node:fs";
import { isAbsolute, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { sourceEvidence } from "./native-evidence.mjs";
const reportName = "frontend-closure.json";
const digest = (bytes) => createHash("sha256").update(bytes).digest("hex");
const slash = (path) => path.replaceAll("\\", "/");
const retired =
  /(?:^|[/+])(?:@vue[/+]|vue(?:-demi)?(?:@|\/|$)|reka-ui(?:@|\/|$))|\.vue(?:\?|$)/i;
const development =
  /(?:^|\/)tests\/|(?:^|\/)docs\/product\/ui-ux\/|(?:^|\/)foundation-catalog\.|(?:^|\/)login-fixture\./;
const markers = ["/__fixture/", "隔离样本与验证工具", "RSS · UI/UX 交互原型"];
export function assertFrontendClosure(modules, files) {
  if (!Array.isArray(modules) || !modules.length)
    throw Error("frontend module inventory missing");
  for (const id of modules) {
    if (typeof id !== "string" || retired.test(id) || development.test(id))
      throw Error(`non-production frontend module: ${id}`);
  }
  for (const [name, bytes] of Object.entries(files)) {
    if (
      retired.test(name) ||
      development.test(name) ||
      /(?:^|\/)\.env(?:\.|$)/.test(name)
    )
      throw Error(`non-production frontend asset: ${name}`);
    if (
      /\.(?:[cm]?js|html|css|json)$/.test(name) &&
      markers.some((marker) => bytes.toString("utf8").includes(marker))
    )
      throw Error(`development fixture in frontend asset: ${name}`);
  }
}
function outputFiles(directory) {
  const files = {};
  const walk = (current, prefix = "") => {
    for (const name of readdirSync(current).sort()) {
      const path = join(current, name),
        key = prefix + name,
        stat = lstatSync(path);
      if (stat.isDirectory()) walk(path, `${key}/`);
      else if (stat.isFile()) {
        // Declaration-only tsc output is emitted after Vite; it is not executable frontend content.
        if (key !== reportName && !/\.d\.ts(?:\.map)?$/.test(key))
          files[key] = readFileSync(path);
      } else throw Error(`unsupported frontend output: ${key}`);
    }
  };
  walk(directory);
  return files;
}
export function verifyFrontendClosure(directory, expectedSource) {
  const report = JSON.parse(readFileSync(join(directory, reportName), "utf8"));
  if (
    report.version !== 1 ||
    !["ui", "desktop"].includes(report.owner) ||
    !/^[a-f0-9]{40}$/.test(report.source?.head ?? "") ||
    !/^[a-f0-9]{64}$/.test(report.source?.sourceSha256 ?? "")
  )
    throw Error("invalid frontend closure report");
  if (
    expectedSource &&
    ["head", "sourceSha256", "cargoLockSha256", "pnpmLockSha256"].some(
      (key) => report.source[key] !== expectedSource[key],
    )
  )
    throw Error("frontend source differs from fixed candidate");
  const files = outputFiles(directory);
  assertFrontendClosure(report.modules, files);
  const actual = Object.fromEntries(
    Object.entries(files).map(([name, bytes]) => [name, digest(bytes)]),
  );
  if (JSON.stringify(actual) !== JSON.stringify(report.files))
    throw Error("frontend output differs from closure inventory");
  return report;
}
/** Build-time evidence from the emitted chunks, including lazy chunks; never a runtime authority. */
export function frontendClosure(owner) {
  let repository, output;
  return {
    name: "rss-production-frontend-closure",
    apply: "build",
    configResolved(config) {
      repository = resolve(config.root, "../..");
      output = resolve(config.root, config.build.outDir);
    },
    writeBundle(_options, bundle) {
      const modules = [
        ...new Set(
          Object.values(bundle)
            .filter((item) => item.type === "chunk")
            .flatMap((item) => [
              ...Object.keys(item.modules),
              ...(item.facadeModuleId ? [item.facadeModuleId] : []),
              ...[...item.imports, ...item.dynamicImports].filter(
                (id) => !bundle[id],
              ),
            ]),
        ),
      ]
        .map((id) => {
          let name = slash(id).replaceAll(slash(repository) + "/", "");
          // Package stores may live outside the checkout; report their module identity, not a user's path.
          if (isAbsolute(name)) {
            const at = name.indexOf("node_modules/");
            name = at >= 0 ? name.slice(at) : `external:${digest(name)}`;
          }
          return name.replaceAll("\0", "virtual:");
        })
        .sort();
      const files = outputFiles(output);
      assertFrontendClosure(modules, files);
      const entry =
        owner === "desktop"
          ? "apps/desktop/src/main.tsx"
          : "packages/ui/runtime.ts";
      if (!modules.some((id) => id === entry))
        throw Error(`frontend entry missing: ${entry}`);
      const report = {
        version: 1,
        owner,
        source: sourceEvidence(repository),
        modules,
        files: Object.fromEntries(
          Object.entries(files).map(([name, bytes]) => [name, digest(bytes)]),
        ),
      };
      writeFileSync(
        join(output, reportName),
        JSON.stringify(report, null, 2) + "\n",
      );
      verifyFrontendClosure(output, report.source);
    },
  };
}
if (
  process.argv[1] &&
  resolve(process.argv[1]) === fileURLToPath(import.meta.url)
) {
  if (
    process.argv[2] !== "--check" ||
    !process.argv[3] ||
    !existsSync(process.argv[3])
  )
    throw Error("usage: frontend-closure.mjs --check <built-dist>");
  const result = verifyFrontendClosure(resolve(process.argv[3]));
  console.log(
    JSON.stringify({
      owner: result.owner,
      source: result.source,
      modules: result.modules.length,
      files: Object.keys(result.files).length,
    }),
  );
}
