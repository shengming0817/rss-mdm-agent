import { activeStage } from "@rss-mdm-agent/ai-contract";
import { Deadline } from "./deadline.js";
import { randomUUID } from "node:crypto";
import {
  boundedJson,
  deliveryFingerprint,
  isId,
  type Budget,
  type Delivery,
  type Event,
  type Namespace,
  type Result,
  type Session,
  type SessionStore,
  type ToolEndpoint,
} from "@rss-mdm-agent/ai-contract";
import {
  defaultLimits,
  fail,
  namespaceKey,
  ok,
} from "@rss-mdm-agent/ai-contract/transitions";

export type Proposal = Parameters<ToolEndpoint["propose"]>[0];
export type ToolReply = Extract<
  Awaited<ReturnType<ToolEndpoint["propose"]>>,
  { ok: true }
>["value"];
export type DeliveryRequest = Extract<
  Event,
  { body: { type: "delivery_requested" } }
>;
export interface DeliveryReceipt {
  readonly receiptRef: string;
  readonly reply: ToolReply;
}
/** Business mapping and authoritative reconciliation belong to the composition root. */
export interface DeliveryRouter {
  prepare(
    namespace: Namespace,
    proposal: Proposal,
  ): Result<{ operationId: string; target: string }>;
  send(
    request: DeliveryRequest,
    budget: Budget,
  ): Promise<Result<DeliveryReceipt>>;
  reconcile(
    request: DeliveryRequest,
    budget: Budget,
  ): Promise<
    Result<
      | { state: "committed"; receipt: DeliveryReceipt }
      | { state: "not_submitted" | "unknown" }
    >
  >;
  /** Called only after the receipt reference has committed locally; must be idempotent. */
  acknowledge(
    request: DeliveryRequest,
    receipt: DeliveryReceipt,
    budget: Budget,
  ): Promise<Result<void>>;
}
type Stored = { delivery: Delivery; event: Event };
const value = <T>(result: Result<T>): T => {
  if (!result.ok) throw new Error(result.error.code);
  return result.value;
};
/** One Host owner's durable outbox. No receiver business state is cached here. */
export class Deliveries {
  private readonly active = new Map<string, Promise<Result<ToolReply>>>();
  private recoveryAfter?: string;
  constructor(
    private readonly store: SessionStore,
    private readonly router: DeliveryRouter,
    private readonly mailbox: <T>(
      n: Namespace,
      fn: () => Promise<T>,
    ) => Promise<T>,
    private readonly changed: (n: Namespace, after: number) => Promise<void>,
    private readonly now = Date.now,
  ) {}
  async propose(
    namespace: Namespace,
    generation: string,
    commandId: string,
    proposal: Proposal,
    budget: Budget,
  ): Promise<Result<ToolReply>> {
    try {
      boundedJson(proposal, defaultLimits);
      const prepared = this.router.prepare(namespace, proposal);
      if (!prepared.ok) return prepared;
      const { operationId, target } = prepared.value;
      if (!isId(operationId) || !isId(target)) return fail("invalid_input");
      const stored = await this.mailbox(namespace, async () => {
        const session = value(await this.store.session(namespace));
        if (
          activeStage(session).binding.generation !== generation ||
          session.status !== "active"
        )
          throw new Error("stale_binding");
        const existing = value(
          await this.store.delivery(namespace, operationId),
        );
        if (existing) {
          if (
            existing.event.body.type !== "delivery_requested" ||
            existing.delivery.target !== target ||
            boundedJson(existing.event.body.proposal, defaultLimits) !==
              boundedJson(proposal, defaultLimits)
          )
            throw new Error("content_conflict");
          return existing;
        }
        const event: DeliveryRequest = {
          schemaVersion: 5,
          kind: "event",
          namespace,
          eventId: randomUUID(),
          sequence: session.lastSequence + 1,
          generation,
          commandId,
          body: {
            type: "delivery_requested",
            operationId,
            target,
            proposal: structuredClone(proposal),
          },
        };
        const delivery: Delivery = {
          schemaVersion: 5,
          kind: "delivery",
          namespace,
          operationId,
          eventId: event.eventId,
          target,
          contentHash: deliveryFingerprint(event, target, defaultLimits),
          retry: "reconcile_first",
          status: "pending",
          attempts: 0,
          nextAttemptAtMs: this.now(),
        };
        await this.commit(session, delivery, [event]);
        return { delivery, event };
      });
      return await this.deliver(stored, budget);
    } catch (error) {
      if (
        error instanceof Error &&
        ["content_conflict", "stale_binding"].includes(error.message)
      )
        return fail(error.message as "content_conflict" | "stale_binding");
      return fail("unavailable", "reconcile_first");
    }
  }
  async recover(budget: Budget): Promise<Result<void>> {
    const deadline = new Deadline(budget);
    try {
      // Advance the page before waiting on receivers. A hung operation cannot monopolize
      // the next sweep; four concurrent attempts keep transport pressure bounded.
      const page = value(
        await deadline.wait(() =>
          this.store.deliveries(4, this.now(), this.recoveryAfter),
        ),
      );
      this.recoveryAfter = page.next;
      const results = await Promise.all(
        page.items.map(async (row) => {
          try {
            const stored = value(
              await deadline.wait(() =>
                this.store.delivery(row.namespace, row.operationId),
              ),
            );
            return stored?.event.body.type === "delivery_requested"
              ? await deadline.wait(() =>
                  this.deliver(stored, deadline.budget()),
                )
              : ok(undefined);
          } catch {
            return fail("unavailable", "reconcile_first");
          }
        }),
      );
      const failure = results.find((result) => !result.ok);
      return failure && !failure.ok ? failure : ok(undefined);
    } catch {
      return fail("unavailable", "reconcile_first");
    } finally {
      deadline.dispose();
    }
  }
  private deliver(stored: Stored, budget: Budget): Promise<Result<ToolReply>> {
    const key = `${namespaceKey(stored.delivery.namespace)}:${stored.delivery.operationId}`;
    const existing = this.active.get(key);
    if (existing) return existing;
    const deadline = new Deadline(budget);
    const task = deadline
      .wait(() => this.attempt(stored, deadline.budget()))
      .catch(() => fail<ToolReply>("unavailable", "reconcile_first"))
      .finally(() => deadline.dispose());
    this.active.set(key, task);
    void task.finally(() => this.active.delete(key));
    return task;
  }
  private async attempt(
    stored: Stored,
    budget: Budget,
  ): Promise<Result<ToolReply>> {
    if (
      budget.signal.aborted ||
      stored.event.body.type !== "delivery_requested"
    )
      return fail("unavailable");
    const request = stored.event as DeliveryRequest;
    const { namespace, operationId } = stored.delivery;
    let receipt: DeliveryReceipt | undefined;
    if (stored.delivery.status !== "pending" || stored.delivery.attempts > 0) {
      const result = await this.router.reconcile(request, budget);
      if (!result.ok) return result;
      if (result.value.state === "committed") receipt = result.value.receipt;
      else if (
        result.value.state === "unknown" ||
        ["receipt_recorded", "delivered"].includes(stored.delivery.status)
      )
        return fail("reconciliation_required", "reconcile_first");
    }
    if (!receipt) {
      await this.mailbox(namespace, async () => {
        const current = value(
          await this.store.delivery(namespace, operationId),
        );
        if (!current || current.delivery.status === "delivered")
          throw new Error("delivery changed");
        const session = value(await this.store.session(namespace));
        await this.commit(
          session,
          {
            ...current.delivery,
            status: "reconciliation_required",
            attempts: current.delivery.attempts + 1,
            nextAttemptAtMs: this.now() + 1000,
          },
          [],
        );
      });
      if (budget.signal.aborted) return fail("unavailable", "reconcile_first");
      const sent = await this.router.send(request, budget);
      if (!sent.ok) return sent;
      receipt = sent.value;
    }
    if (!isId(receipt.receiptRef)) return fail("invalid_input");
    boundedJson(receipt.reply, defaultLimits);
    await this.mailbox(namespace, async () => {
      const current = value(await this.store.delivery(namespace, operationId));
      if (!current) throw new Error("delivery absent");
      if (["receipt_recorded", "delivered"].includes(current.delivery.status))
        return;
      const session = value(await this.store.session(namespace));
      const event: Event = {
        schemaVersion: 5,
        kind: "event",
        namespace,
        eventId: randomUUID(),
        generation: activeStage(session).binding.generation,
        commandId: request.commandId,
        sequence: session.lastSequence + 1,
        body: {
          type: "delivery_recorded",
          operationId,
          target: current.delivery.target,
          contentHash: current.delivery.contentHash,
          receiptRef: receipt!.receiptRef,
        },
      };
      await this.commit(
        session,
        { ...current.delivery, status: "receipt_recorded" },
        [event],
      );
    });
    const acknowledged = await this.router.acknowledge(
      request,
      receipt,
      budget,
    );
    if (!acknowledged.ok) return acknowledged;
    await this.mailbox(namespace, async () => {
      const current = value(await this.store.delivery(namespace, operationId));
      if (!current || current.delivery.status === "delivered") return;
      await this.commit(
        value(await this.store.session(namespace)),
        { ...current.delivery, status: "delivered" },
        [],
      );
    });
    return ok(receipt.reply);
  }
  private async commit(session: Session, delivery: Delivery, events: Event[]) {
    value(
      await this.store.commit({
        namespace: session.namespace,
        expectedRevision: session.revision,
        expectedGeneration: activeStage(session).binding.generation,
        session: {
          ...session,
          revision: session.revision + 1,
          lastSequence: session.lastSequence + events.length,
        },
        commands: [],
        interactions: [],
        surfaces: [],
        events,
        deliveries: [delivery],
        nowMs: this.now(),
      }),
    );
    await this.changed(session.namespace, session.lastSequence);
  }
}
