import type {
  Binding,
  Capabilities,
  ContextStage,
  Namespace,
  Session,
} from "./wire.js";
import { ContractError } from "./codec.js";

/** Pure record access. A stage is data, never proof that its provider was admitted. */
export function activeStage(session: Session): ContextStage {
  const stage = session.stages.find(
    (row) => row.stageId === session.currentStageId,
  );
  if (!stage) throw new ContractError("context");
  return stage;
}
export function productSession(
  namespace: Namespace,
  selectedConnectionId?: string,
): Session {
  return {
    schemaVersion: 5,
    kind: "session",
    namespace: structuredClone(namespace),
    revision: 0,
    lastSequence: 0,
    status: "active",
    stages: [],
    ...(selectedConnectionId ? { selectedConnectionId } : {}),
  };
}
export function providerStage(
  binding: Binding,
  capabilities: Capabilities,
  configRevision = 1,
): ContextStage {
  return {
    stageId: binding.generation,
    connectionId: binding.config.id,
    configRevision,

    binding: structuredClone(binding),
    capabilities: structuredClone(capabilities),
  };
}
export function startStage(session: Session, stage: ContextStage): Session {
  if (session.stages.some((row) => row.stageId === stage.stageId))
    throw new ContractError("context");
  const { freshContext: _, ...previous } = session;
  return {
    ...previous,
    currentStageId: stage.stageId,
    selectedConnectionId: stage.connectionId,
    stages: [...session.stages, structuredClone(stage)],
  };
}
/** Update the active incarnation without overwriting earlier context phases. */
export function replaceStage(
  session: Session,
  binding?: Binding,
  capabilities?: Capabilities,
): Session {
  const current = activeStage(session);
  return {
    ...session,
    stages: session.stages.map((row) =>
      row.stageId === current.stageId
        ? {
            ...row,
            binding: structuredClone(binding ?? row.binding),
            capabilities: structuredClone(capabilities ?? row.capabilities),
          }
        : row,
    ),
  };
}
