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
  ): Result<{
    operationId: string;
    target: string;
    permission: "none" | "ask";
  }>;
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
export type DeliveryAuthorizer = (
  request: {
    namespace: Namespace;
    generation: string;
    commandId: string;
    operationId: string;
    proposal: Proposal;
  },
  budget: Budget,
) => Promise<Result<"allowed" | "rejected">>;
type Stored = { delivery: Delivery; event: Event };
const value = <T>(result: Result<T>): T => {
  if (!result.ok) throw new Error(result.error.code);
  return result.value;
};
/** One Host owner's durable outbox. No receiver business state is cached here. */
export class Deliveries {
  private readonly active = new Map<
    string,
    { fingerprint: string; task: Promise<Result<ToolReply>> }
  >();
  private recoveryAfter?: string;
  constructor(
    private readonly store: SessionStore,
    private readonly router: DeliveryRouter,
    private readonly mailbox: <T>(
      n: Namespace,
      fn: () => Promise<T>,
    ) => Promise<T>,
    private readonly changed: (n: Namespace, after: number) => Promise<void>,
    private readonly now: () => number,
    private readonly authorize: DeliveryAuthorizer,
    private readonly current: (
      namespace: Namespace,
      generation: string,
    ) => boolean,
  ) {}
  async propose(
    namespace: Namespace,
    generation: string,
    commandId: string,
    proposal: Proposal,
    budget: Budget,
  ): Promise<Result<ToolReply>> {
    try {
      proposal = JSON.parse(boundedJson(proposal, defaultLimits)) as Proposal;
      const prepared = this.router.prepare(namespace, proposal);
      if (!prepared.ok) return prepared;
      const { operationId, target, permission } = prepared.value;
      if (
        !isId(operationId) ||
        !isId(target) ||
        !["none", "ask"].includes(permission)
      )
        return fail("invalid_input");
      const fingerprint = boundedJson({ target, proposal }, defaultLimits);
      return await this.coordinate(
        namespace,
        operationId,
        fingerprint,
        budget,
        async (deadline) => {
          const existing = await this.mailbox(namespace, () =>
            this.existing(namespace, operationId, fingerprint),
          );
          if (existing) return this.attempt(existing, deadline.budget());
          await this.mailbox(namespace, () =>
            this.live(namespace, generation, commandId, deadline),
          );
          if (permission === "ask") {
            const answer = await this.authorize(
              {
                namespace,
                generation,
                commandId,
                operationId,
                proposal: structuredClone(proposal),
              },
              deadline.budget(),
            );
            if (!answer.ok) return answer;
            if (answer.value === "rejected")
              return ok({
                disposition: "rejected",
                text: "用户拒绝本次 AI 工具调用。",
              });
            if (answer.value !== "allowed") return fail("unavailable");
          }
          const stored = await this.mailbox(namespace, async () => {
            const session = await this.live(
              namespace,
              generation,
              commandId,
              deadline,
            );
            const replay = await this.existing(
              namespace,
              operationId,
              fingerprint,
            );
            if (replay) return replay;
            deadline.check();
            const event: DeliveryRequest = {
              schemaVersion: 7,
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
                proposal,
              },
            };
            const delivery: Delivery = {
              schemaVersion: 7,
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
          // Already inside the operation's single flight: calling deliver here would await itself.
          return this.attempt(stored, deadline.budget());
        },
      );
    } catch {
      return fail("unavailable", "reconcile_first");
    }
  }
  private async existing(
    namespace: Namespace,
    operationId: string,
    fingerprint: string,
  ): Promise<Stored | undefined> {
    const stored = value(await this.store.delivery(namespace, operationId));
    if (
      stored &&
      (stored.event.body.type !== "delivery_requested" ||
        boundedJson(
          {
            target: stored.delivery.target,
            proposal: stored.event.body.proposal,
          },
          defaultLimits,
        ) !== fingerprint)
    )
      throw new Error("content_conflict");
    return stored ?? undefined;
  }
  private async live(
    namespace: Namespace,
    generation: string,
    commandId: string,
    deadline: Deadline,
  ): Promise<Session> {
    const session = value(await this.store.session(namespace));
    const command = value(await this.store.command(namespace, commandId));
    deadline.check();
    if (
      !this.current(namespace, generation) ||
      session.status !== "active" ||
      activeStage(session).binding.generation !== generation ||
      command.command.input.type !== "prompt" ||
      !["running", "dispatching"].includes(command.state) ||
      command.dispatch?.observerGeneration !== generation
    )
      throw new Error("stale_binding");
    return session;
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
    if (stored.event.body.type !== "delivery_requested")
      return Promise.resolve(fail("invalid_input"));
    return this.coordinate(
      stored.delivery.namespace,
      stored.delivery.operationId,
      boundedJson(
        {
          target: stored.delivery.target,
          proposal: stored.event.body.proposal,
        },
        defaultLimits,
      ),
      budget,
      (deadline) => this.attempt(stored, deadline.budget()),
    );
  }
  private coordinate(
    namespace: Namespace,
    operationId: string,
    fingerprint: string,
    budget: Budget,
    action: (deadline: Deadline) => Promise<Result<ToolReply>>,
  ): Promise<Result<ToolReply>> {
    const key = `${namespaceKey(namespace)}:${operationId}`;
    const existing = this.active.get(key);
    if (existing && existing.fingerprint !== fingerprint)
      return Promise.resolve(fail("content_conflict"));
    const failure = (error: unknown): Result<ToolReply> =>
      error instanceof Error &&
      ["content_conflict", "stale_binding"].includes(error.message)
        ? fail<ToolReply>(error.message as "content_conflict" | "stale_binding")
        : fail<ToolReply>("unavailable", "reconcile_first");
    let operation = existing?.task;
    if (!existing) {
      // ref: golang/sync singleflight.go doCall: ownership ends with fn, not a waiter.
      // Abort is advisory: a router or authorizer may still be doing work afterwards.
      const owner = new Deadline(budget);
      const task = Promise.resolve()
        .then(() => {
          owner.check();
          return action(owner);
        })
        .catch(failure)
        .finally(() => {
          owner.dispose();
          if (this.active.get(key)?.task === task) this.active.delete(key);
        });
      this.active.set(key, { fingerprint, task });
      operation = task;
    }
    const waiter = new Deadline(budget);
    return waiter
      .wait(() => operation!)
      .catch(failure)
      .finally(() => waiter.dispose());
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
        schemaVersion: 7,
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
