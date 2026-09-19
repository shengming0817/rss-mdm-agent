import type { Context } from "@deepseek-ai/cordis";
import type {} from "@deepseek-ai/dsh-tools";
/** Closed tool inventory and a monotonic deny guard owned for the entire Context. */
export function sealTools(
  ctx: Context,
  sessionId: string,
  allowed: readonly string[],
  onDrift: () => void,
): () => void {
  const definitions = new Map(
    allowed.map((name) => [name, ctx.tools.get(name)]),
  );
  if ([...definitions.values()].some((v) => !v)) throw Error("missing tool");
  let live = true;
  const verify = () => {
    if (!live || allowed.some((n) => ctx.tools.get(n) !== definitions.get(n)))
      throw Error("profile drift");
  };
  const remove = ctx.tools.guard((exec) => {
    try {
      verify();
    } catch {
      return "incarnation drift";
    }
    return exec.parent ||
      exec.agent?.id !== sessionId ||
      !allowed.includes(exec.name) ||
      ctx.tools.get(exec.name, exec.agent) !== definitions.get(exec.name)
      ? "tool denied"
      : undefined;
  });
  ctx.effect(
    () => () => {
      live = false;
      remove();
    },
    "rss deny guard",
  );
  ctx.on("tools/change", () => {
    if (live) {
      live = false;
      onDrift();
    }
  });
  return verify;
}
