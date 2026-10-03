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
  const fetch = vi.fn().mockResolvedValue({ ok: true, json: async () => ({}) });
  vi.stubGlobal("fetch", fetch);
  try {
    const offer = fixtures.offer;
    await ports.assistantServices.confirmTask!(offer as never);
    expect(fetch.mock.calls[0][0]).toBe("/__fixture/confirm");
    expect(JSON.parse(fetch.mock.calls[0][1].body)).toEqual({
      request: offer.request,
      task: offer.task,
      attempt: offer.attempt,
      revision: offer.revision,
    });
    await ports.assistantServices.cancelTask!(offer.request);
    expect(fetch.mock.calls[1][0]).toBe("/__fixture/cancel");
    expect(JSON.parse(fetch.mock.calls[1][1].body)).toEqual({
      requestId: offer.request,
    });
  } finally {
    vi.unstubAllGlobals();
  }
});
