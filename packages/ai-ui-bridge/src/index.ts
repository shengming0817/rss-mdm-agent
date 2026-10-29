import { RendererError } from "./errors.js";
import type { SurfaceRendererHandle, RendererOptions } from "./renderer.js";
export type { RendererOptions, SurfaceRendererHandle } from "./renderer.js";
export { RendererError } from "./errors.js";
export type { RendererErrorCode } from "./errors.js";
export { questions } from "./questions.js";
export type { Question } from "./questions.js";
export type RendererFactory = (
  container: HTMLElement,
  options: RendererOptions,
) => Promise<SurfaceRendererHandle>;
/** Lazy import makes renderer load failure a display error; text and ACP remain usable. */
export const createSurfaceRenderer: RendererFactory = async (
  container,
  options,
) => {
  try {
    const { SurfaceRenderer } = await import("./renderer.js");
    return new SurfaceRenderer(container, options);
  } catch {
    throw new RendererError("renderer_load");
  }
};

export { RuntimeSurface, useSessionView } from "./runtime-surface.js";
export type { RuntimeSurfaceProps } from "./runtime-surface.js";
export {
  createOperationController,
  OperationDrafts,
} from "./operations/controller.js";
export type { OperationController } from "./operations/controller.js";
export type { NodeMount } from "./operations/surface.js";
