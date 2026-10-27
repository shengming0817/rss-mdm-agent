import { expect, it, vi } from "vitest";
import { assertFixtureAssembly } from "./assembly";
import { createFixturePorts } from "../../../tests/assistant/ports";
import fixtures from "../../../tests/assistant/execution-fixtures.json";
it("requires every fixture port and sends AI confirmation/cancellation to the exact execution owner", async () => {
  const ports = createFixturePorts();
  assertFixtureAssembly(ports);
  for (const port of ["identity", "host", "service"] as const) {
    expect(() =>
      assertFixtureAssembly({
        ...ports,
        environment: { ...ports.environment, [port]: undefined } as never,
      }),
    ).toThrow("incomplete");
  }
  for (const method of ["confirmTask", "cancelTask", "taskDetails"] as const) {
    expect(() =>
      assertFixtureAssembly({
        ...ports,
        assistantServices: { ...ports.assistantServices, [method]: undefined },
      }),
    ).toThrow("incomplete");
  }
  for (const method of [
    "registration",
    "requestRegistration",
    "mdmStatus",
  ] as const) {
    expect(() =>
      assertFixtureAssembly({
        ...ports,
        environment: {
          ...ports.environment,
          service: {
            ...ports.environment.service,
            [method]: undefined,
          } as never,
        },
      }),
    ).toThrow("incomplete");
  }
  const fetch = vi.fn(async (url: string, ..._options: unknown[]) => ({
    ok: true,
    json: async () =>
      url.endsWith("task-detail")
        ? {
            reference: { kind: "request", request: fixtures.offer.request },
            scope: "fixture-scope",
            revision: { owner: 1, transfers: [] },
          }
        : url.endsWith("task-control")
          ? { disposition: "confirmed" }
          : {},
  }));
  vi.stubGlobal("fetch", fetch);
  try {
    const offer = fixtures.offer;
    await ports.assistantServices.confirmTask!(offer as never);
    expect(fetch.mock.calls[0][0]).toBe("/__fixture/confirm");
    expect(JSON.parse((fetch.mock.calls[0] as any)[1].body)).toEqual({
      request: offer.request,
      task: offer.task,
      attempt: offer.attempt,
      revision: offer.revision,
    });
    await ports.assistantServices.cancelTask!(offer.request);
    expect(fetch.mock.calls[1][0]).toBe("/__fixture/task-detail");
    expect(fetch.mock.calls[2][0]).toBe("/__fixture/task-control");
    expect(JSON.parse((fetch.mock.calls[2] as any)[1].body)).toMatchObject({
      reference: { kind: "request", request: offer.request },
      scope: "fixture-scope",
      expectedRevision: { owner: 1, transfers: [] },
      action: "cancel",
    });
  } finally {
    vi.unstubAllGlobals();
  }
});
