import { expect, it, vi } from "vitest";
import { readFileSync } from "node:fs";
import { URL as FileURL } from "node:url";
const source = readFileSync(
  new FileURL(
    "../../src-tauri/src/composition/employee-browser.js",
    import.meta.url,
  ),
  "utf8",
);
const contract = JSON.parse(
  readFileSync(
    new FileURL(
      "../../../../tests/agent-protocol/http-v1.json",
      import.meta.url,
    ),
    "utf8",
  ),
);
const origin = "https://org.example.test";
const intent = {
  operationId: "11111111-1111-4111-8111-111111111111",
  expectedRevision: 3,
  action: { kind: "reset_factors" },
};
const input = {
  origin,
  tenant: "tenant",
  device: "device",
  binding: "binding",
  principal: "alice",
  change: intent,
  returnTarget: "employee-recovery",
};
type Route = { method: string; path: string };
it.each(["agent", "da"])(
  "uses the configured %s OIDC route and the original purpose",
  async (product) => {
    document.body.replaceChildren();
    Object.defineProperty(HTMLDialogElement.prototype, "showModal", {
      configurable: true,
      value: vi.fn(),
    });
    const route = (contract.employeeBrowser[product] as Route[]).find(
      (r) => r.method === "POST" && r.path.endsWith("/step-up/oidc/{provider}"),
    );
    expect(route).toBeDefined();
    const path = route!.path
      .replace("{tenant}", input.tenant)
      .replace("{device}", input.device)
      .replace("{id}", input.binding)
      .replace("{provider}", "org-idp");
    const assign = vi.fn();
    const fetcher = vi.fn(async (url: string, options: RequestInit) => {
      if (url.endsWith("/session"))
        return Response.json({
          identity: { principalId: "alice" },
          csrfToken: "test-csrf",
        });
      if (url.endsWith("/login-options"))
        return Response.json({
          providers: [{ label: "组织", providerId: "org-idp" }],
        });
      expect([options.method, url]).toEqual([route!.method, path]);
      expect(JSON.parse(String(options.body))).toEqual({
        change: intent,
        returnTarget: input.returnTarget,
      });
      expect(options.headers).toHaveProperty("x-csrf-token", "test-csrf");
      return Response.json({
        authorizationUrl: "https://idp.example.test/authorize",
      });
    });
    const start = new Function(
      "window",
      "document",
      "location",
      "fetch",
      "return (" + source + ")",
    )(window, document, { origin, assign }, fetcher);
    await start(input);
    const provider = Array.from(document.querySelectorAll("button")).find(
      (b) => b.textContent === "使用 组织 验证",
    );
    expect(provider).toBeDefined();
    provider!.click();
    await vi.waitFor(() =>
      expect(assign).toHaveBeenCalledWith("https://idp.example.test/authorize"),
    );
    expect(fetcher).toHaveBeenCalledTimes(4);
    document.body.replaceChildren();
  },
);

it("uses the binary WebAuthn API when the JSON convenience methods are absent", async () => {
  document.body.replaceChildren();
  Object.defineProperty(HTMLDialogElement.prototype, "showModal", {
    configurable: true,
    value: vi.fn(),
  });
  const get = vi.fn(async ({ publicKey }) => {
    expect(Array.from(publicKey.challenge)).toEqual([251, 255]);
    expect(Array.from(publicKey.allowCredentials[0].id)).toEqual([1, 2]);
    return {
      type: "public-key",
      id: "AQI",
      rawId: new Uint8Array([1, 2]),
      response: {
        clientDataJSON: new Uint8Array([3]),
        authenticatorData: new Uint8Array([4]),
        signature: new Uint8Array([251, 255]),
        userHandle: null,
      },
      getClientExtensionResults: () => ({}),
    };
  });
  const finish = vi.fn();
  const fetcher = async (url: string, options: RequestInit) => {
    if (url.endsWith("/session"))
      return Response.json({
        identity: { principalId: "alice" },
        csrfToken: "csrf",
      });
    if (url.endsWith("/step-up"))
      return Response.json({
        id: "challenge",
        options: {
          publicKey: {
            challenge: "-_8",
            allowCredentials: [{ type: "public-key", id: "AQI" }],
          },
        },
      });
    expect(url.endsWith("/webauthn/authenticate/finish")).toBe(true);
    finish(JSON.parse(String(options.body)));
    return Response.json({ expiresAt: 3 });
  };
  await new Function(
    "window",
    "document",
    "location",
    "fetch",
    "navigator",
    "return (" + source + ")",
  )(window, document, { origin }, fetcher, { credentials: { get } })({
    ...input,
    returnTarget: null,
  });
  document.querySelector("button")!.click();
  await vi.waitFor(() =>
    expect(finish).toHaveBeenCalledWith({
      id: "challenge",
      response: {
        type: "public-key",
        id: "AQI",
        rawId: "AQI",
        response: {
          clientDataJSON: "Aw",
          authenticatorData: "BA",
          signature: "-_8",
          userHandle: null,
        },
        clientExtensionResults: {},
      },
    }),
  );
  document.body.replaceChildren();
});
