import { useEffect, useMemo } from "react";
import { Button, Select } from "antd";
import { useSnapshot } from "valtio";
import { App } from "../../apps/desktop/src/App";
import {
  DesktopProvider,
  useDesktop,
} from "../../apps/desktop/src/state/react";
import { createFixturePorts } from "./ports";
import { configurePageFixture } from "./page-fixture";
/** Explicit development-only assembly of production shell/pages with isolated ports. */
export function DesktopFixture() {
  const parameters = new URLSearchParams(window.location.search),
    scene = parameters.get("scene") ?? "normal",
    platform = parameters.get("platform") === "windows" ? "windows" : "macos";
  const assembly = useMemo(() => {
    const ports = createFixturePorts();
    const pages = configurePageFixture(ports, scene);
    if (scene === "registration-wait") {
      const read = ports.environment.service.requestRegistration;
      let first = true,
        release: (() => void) | undefined;
      ports.environment.service.requestRegistration = async (input) => {
        const status = await read(input);
        if (first) {
          first = false;
          await new Promise<void>((resolve) => {
            release = resolve;
          });
        }
        return status;
      };
      ports.environment.service.cancelRegistration = async () => {
        release?.();
      };
    }
    if (scene === "task-error" && ports.environment.local.tasks) {
      const read = ports.environment.local.tasks.detail;
      let first = true;
      ports.environment.local.tasks.detail = async (reference) => {
        if (first) {
          first = false;
          throw Error("fixture original detail unavailable");
        }
        return read(reference);
      };
    }
    if (scene === "error") {
      ports.environment.device.read = async () => {
        throw Error("fixture read denied");
      };
      if (ports.environment.local.software)
        ports.environment.local.software.read = async () => {
          throw Error("fixture inventory unavailable");
        };
      if (ports.environment.local.settings)
        ports.environment.local.settings = async () => {
          throw Error("fixture settings unavailable");
        };
    }
    if (ports.environment.local.software) {
      const read = ports.environment.local.software.read;
      ports.environment.local.software.read = async (value) => {
        const page = await read(value);
        if (scene === "empty") return { ...page, items: [] };
        if (scene === "partial")
          return { ...page, quality: "partial", enumerationComplete: false };
        return page;
      };
    }
    return { ports, pages };
  }, [scene]);
  // Publish only the committed assembly; StrictMode may discard a memo calculation.
  useEffect(() => {
    Object.assign(window, { rssPageFixture: assembly.pages });
    return () => {
      Object.assign(window, { rssPageFixture: undefined });
    };
  }, [assembly]);
  return (
    <DesktopProvider input={assembly.ports} platform={platform}>
      <App />
      <FixtureControls />
    </DesktopProvider>
  );
}
function FixtureControls() {
  const desktop = useDesktop(),
    state = useSnapshot(desktop.state),
    login = desktop.fixture?.account;
  useEffect(() => {
    Object.assign(window, { rssDesktop: desktop });
  }, [desktop]);
  return (
    <details
      className="fixture-controls"
      style={{
        position: "fixed",
        right: 12,
        bottom: 36,
        zIndex: 2000,
        maxWidth: 380,
        padding: 8,
        border: "1px solid #aaa",
        borderRadius: 8,
        background: "#fff",
        color: "#222",
        fontSize: 11,
      }}
    >
      <summary>隔离样本与验证工具</summary>
      <p>仅开发装配 · 无生产认证或系统执行</p>
      <Select
        aria-label="验收场景"
        defaultValue={
          new URLSearchParams(window.location.search).get("scene") ?? "normal"
        }
        options={[
          "normal",
          "empty",
          "partial",
          "error",
          "offline",
          "readonly",
        ].map((value) => ({
          value,
          label: value,
        }))}
        onChange={(value) => {
          const parameters = new URLSearchParams(window.location.search);
          parameters.set("scene", value);
          window.location.search = parameters.toString();
        }}
      />
      <Select
        aria-label="验收主题"
        value={state.theme}
        options={[
          { value: "system", label: "跟随系统" },
          { value: "light", label: "浅色" },
          { value: "dark", label: "深色" },
        ]}
        onChange={(value) => desktop.setTheme(value)}
      />
      <div>
        <Button onClick={() => login?.scan()}>模拟扫码</Button>
        <Button onClick={() => login?.expire()}>二维码过期</Button>
        <Button onClick={() => login?.reject()}>拒绝扫码</Button>
      </div>
      <Select
        aria-label="客户端场景"
        defaultValue="normal"
        options={[
          "normal",
          "signed_out",
          "missing",
          "mismatch",
          "expired",
          "error",
        ].map((value) => ({ value, label: value }))}
        onChange={(value) =>
          login?.scenario(
            value as Parameters<NonNullable<typeof login>["scenario"]>[0],
          )
        }
      />
    </details>
  );
}
