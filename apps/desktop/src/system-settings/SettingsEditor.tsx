import { useEffect, useState } from "react";
import { Alert, Button, Input, InputNumber, Select, Switch } from "antd";
import { useSnapshot } from "valtio";
import type { Snapshot } from "valtio/vanilla";
import type {
  LocalPreparation,
  ProxyEndpoint,
  SettingsPatch,
  SettingsSection,
  SettingsTarget,
} from "../self-service/types";
import type { SystemSettingsController } from "./controller";
import { deepClone } from "valtio/vanilla/utils";
import { labels } from "./presentation";

export function SettingsEditor({
  owner,
  target,
  section,
  revision,
  disabled,
  entry,
  onEntryUsed,
}: {
  owner: SystemSettingsController;
  target: Snapshot<SettingsTarget>;
  section: SettingsSection;
  revision: string;
  disabled: boolean;
  entry?: Snapshot<Extract<LocalPreparation, { kind: "settings" }>>;
  onEntryUsed?(): void;
}) {
  const initial = (): SettingsPatch | null => {
    if (
      section === "network" &&
      target.fields.some(
        (field) =>
          ["http", "https", "socks", "bypass"].includes(field.key) &&
          field.writable,
      )
    )
      return {
        kind: "proxy",
        target: target.id,
        http: null,
        https: null,
        socks: null,
        bypass: null,
      };
    if (section === "display" && target.fields.some((field) => field.writable))
      return { kind: "display", target: target.id, mode: null, origin: null };
    if (section === "power" && target.fields.some((field) => field.writable))
      return {
        kind: "power",
        target: target.id,
        mode: null,
        displayOffMinutes: null,
        sleepMinutes: null,
      };
    return null;
  };
  const [edit, setEdit] = useState<{
      patch: SettingsPatch;
      expected: string;
      request?: string;
      bypassText?: string;
      origin?: { x: number | null; y: number | null };
    } | null>(null),
    state = useSnapshot(owner.state),
    result = state.observations.get(section),
    draft = edit?.patch ?? initial(),
    expected = edit?.expected ?? revision,
    dirty = !!edit,
    writable = (key: string) =>
      !disabled &&
      !!target.fields.find((field) => field.key === key && field.writable),
    value = (key: string) =>
      target.fields.find((field) => field.key === key)?.value;
  useEffect(() => {
    if (
      edit?.request &&
      result?.request === edit.request &&
      result.finished &&
      result.effect === "applied"
    ) {
      setEdit((current) => (current === edit ? null : current));
    }
  }, [edit?.request, result?.request, result?.finished, result?.effect]);
  if (!draft) return null;
  const update = (
      patch: SettingsPatch,
      values: Partial<
        Pick<NonNullable<typeof edit>, "bypassText" | "origin">
      > = {},
    ) => {
      setEdit({ ...edit, ...values, patch, expected, request: undefined });
    },
    reset = () => {
      setEdit(null);
    };
  const rawOrigin = value("origin"),
    observedOrigin =
      rawOrigin &&
      typeof rawOrigin === "object" &&
      "x" in rawOrigin &&
      "y" in rawOrigin &&
      typeof rawOrigin.x === "number" &&
      typeof rawOrigin.y === "number"
        ? { x: rawOrigin.x, y: rawOrigin.y }
        : { x: null, y: null },
    origin =
      edit?.origin ??
      (draft.kind === "display" ? draft.origin : null) ??
      observedOrigin,
    validCoordinate = (coordinate: number | null): coordinate is number =>
      coordinate != null &&
      Number.isInteger(coordinate) &&
      coordinate >= -2147483648 &&
      coordinate <= 2147483647,
    invalidOrigin =
      !!edit?.origin &&
      (!validCoordinate(origin.x) || !validCoordinate(origin.y)),
    restricted = Object.entries(draft).some(
      ([key, current]) =>
        !["kind", "target"].includes(key) && current !== null && !writable(key),
    ),
    bypass = Array.isArray(value("bypass"))
      ? (value("bypass") as readonly string[])
      : [];
  return (
    <div className="rss-system-editor rss-stack">
      {entry?.patch?.target === target.id && (
        <Alert
          type="info"
          title="本机入口带有待核对参数，尚未执行。"
          description={
            <Button
              disabled={disabled}
              onClick={() => {
                if (entry.patch && entry.patch.kind === draft.kind) {
                  setEdit({
                    patch: deepClone(entry.patch) as SettingsPatch,
                    expected: entry.expected ?? revision,
                  });
                  onEntryUsed?.();
                }
              }}
            >
              替换草稿为入口参数
            </Button>
          }
        />
      )}
      {expected !== revision &&
        dirty &&
        !(
          edit?.request &&
          result?.request === edit.request &&
          result.effect === "preview"
        ) && (
          <Alert
            type="warning"
            title="观察版本已变化；保存仍核对开始编辑时的版本。请先重新核对草稿。"
          />
        )}
      {draft.kind === "proxy" ? (
        <>
          {(["http", "https", "socks"] as const)
            .filter((key) =>
              target.fields.some(
                (field) => field.key === key && field.writable,
              ),
            )
            .map((protocol) => {
              const raw = value(protocol),
                original =
                  raw &&
                  typeof raw === "object" &&
                  "host" in raw &&
                  "port" in raw &&
                  "enabled" in raw
                    ? (deepClone(raw) as ProxyEndpoint)
                    : { host: "", port: 0, enabled: false },
                endpoint = draft[protocol] ?? original,
                change = (patch: Partial<ProxyEndpoint>) =>
                  update({ ...draft, [protocol]: { ...endpoint, ...patch } });
              return (
                <fieldset className="rss-proxy-editor" key={protocol}>
                  <legend>{labels[protocol]}</legend>
                  <div className="rss-setting-row">
                    <span>启用 {protocol.toUpperCase()} 代理</span>
                    <Switch
                      aria-label={`启用 ${protocol.toUpperCase()} 代理`}
                      disabled={!writable(protocol)}
                      checked={endpoint.enabled}
                      onChange={(enabled) => change({ enabled })}
                    />
                  </div>
                  <div className="rss-form-grid">
                    <label className="rss-field">
                      <span>服务器地址</span>
                      <Input
                        aria-label={`${protocol.toUpperCase()} 代理服务器`}
                        disabled={!writable(protocol)}
                        placeholder="proxy.example.com"
                        value={endpoint.host}
                        onChange={(event) =>
                          change({ host: event.target.value })
                        }
                      />
                    </label>
                    <label className="rss-field">
                      <span>端口</span>
                      <InputNumber
                        aria-label={`${protocol.toUpperCase()} 代理端口`}
                        disabled={!writable(protocol)}
                        min={0}
                        max={65535}
                        precision={0}
                        value={endpoint.port}
                        onChange={(port) => change({ port: port ?? 0 })}
                      />
                    </label>
                  </div>
                </fieldset>
              );
            })}
          {target.fields.some((field) => field.key === "bypass") && (
            <label className="rss-field">
              <span>绕过代理的地址（每行一个）</span>
              <Input.TextArea
                aria-label={`${target.name} 绕过代理的地址`}
                disabled={!writable("bypass")}
                autoSize={{ minRows: 3, maxRows: 6 }}
                value={edit?.bypassText ?? (draft.bypass ?? bypass).join("\n")}
                onChange={(event) => {
                  if (!writable("bypass")) return;
                  const text = event.target.value,
                    entries = text
                      .split(/\r?\n/)
                      .map((row) => row.trim())
                      .filter(Boolean);
                  update(
                    {
                      ...draft,
                      bypass:
                        JSON.stringify(entries) === JSON.stringify(bypass)
                          ? null
                          : entries,
                    },
                    { bypassText: text },
                  );
                }}
              />
            </label>
          )}
          <p className="rss-muted">
            只提交本次编辑的手动代理和绕过列表；清空列表会移除全部绕过地址。自动配置、自动发现与其它协议保持原值。
          </p>
        </>
      ) : draft.kind === "display" ? (
        <>
          {target.options.length > 0 && (
            <label className="rss-field">
              <span>分辨率与缩放模式</span>
              <Select
                aria-label={`${target.name} 显示模式`}
                disabled={!writable("mode")}
                value={
                  draft.mode ??
                  (typeof value("mode") === "string"
                    ? (value("mode") as string)
                    : undefined)
                }
                options={target.options.map((option) => ({
                  label: option.label,
                  value: option.id,
                }))}
                onChange={(mode) => update({ ...draft, mode })}
              />
            </label>
          )}
          {target.fields.some((field) => field.key === "origin") && (
            <div className="rss-form-grid">
              {(["x", "y"] as const).map((axis) => (
                <label className="rss-field" key={axis}>
                  <span>{axis === "x" ? "桌面横坐标" : "桌面纵坐标"}</span>
                  <InputNumber
                    aria-label={`${target.name} 桌面${axis === "x" ? "横" : "纵"}坐标`}
                    min={-2147483648}
                    max={2147483647}
                    precision={0}
                    disabled={!writable("origin")}
                    value={axis === "x" ? origin.x : origin.y}
                    onChange={(coordinate) => {
                      if (!writable("origin")) return;
                      const next = { ...origin, [axis]: coordinate };
                      update(
                        {
                          ...draft,
                          origin:
                            validCoordinate(next.x) && validCoordinate(next.y)
                              ? { x: next.x, y: next.y }
                              : null,
                        },
                        { origin: next },
                      );
                    }}
                  />
                </label>
              ))}
            </div>
          )}
          {invalidOrigin && (
            <Alert type="warning" title="请填写完整的整数坐标后再保存。" />
          )}
          <p className="rss-muted">
            应用后由系统服务保留预览期限；未保留时由原执行器恢复配置。
          </p>
        </>
      ) : (
        <>
          {(["displayOffMinutes", "sleepMinutes"] as const)
            .filter((key) =>
              target.fields.some(
                (field) => field.key === key && field.writable,
              ),
            )
            .map((key) => (
              <label className="rss-field" key={key}>
                <span>{labels[key]} · 0 表示从不</span>
                <InputNumber
                  aria-label={`${target.name} ${labels[key]}`}
                  min={0}
                  max={1440}
                  precision={0}
                  disabled={!writable(key)}
                  value={
                    draft[key] ??
                    (typeof value(key) === "number"
                      ? (value(key) as number)
                      : null)
                  }
                  onChange={(minutes) => update({ ...draft, [key]: minutes })}
                />
              </label>
            ))}
          {target.options.length > 0 && (
            <label className="rss-field">
              <span>电源模式</span>
              <Select
                aria-label={`${target.name} 电源模式`}
                disabled={!writable("mode")}
                value={
                  draft.mode ??
                  (typeof value("mode") === "string"
                    ? (value("mode") as string)
                    : undefined)
                }
                options={target.options.map((option) => ({
                  label: option.label,
                  value: option.id,
                }))}
                onChange={(mode) => update({ ...draft, mode })}
              />
            </label>
          )}
        </>
      )}
      {dirty && restricted && !disabled && (
        <Alert
          type="warning"
          title="草稿包含当前不可修改的字段，请还原草稿后重新核对。"
        />
      )}
      <div className="rss-settings-actions">
        <small>
          {edit?.request && state.pending.has(section)
            ? "原操作结果待核实"
            : dirty
              ? "有尚未保存的修改"
              : "修改仅在保存后提交"}
        </small>
        <Button disabled={disabled} onClick={reset}>
          还原草稿
        </Button>
        <Button
          type="primary"
          disabled={disabled || !dirty || invalidOrigin || restricted}
          onClick={() => {
            const submitted = edit;
            if (!submitted) return;
            void owner
              .save(submitted.patch, submitted.expected)
              .then((request) => {
                if (request)
                  setEdit((current) =>
                    current === submitted ? { ...current, request } : current,
                  );
              });
          }}
        >
          保存更改
        </Button>
      </div>
    </div>
  );
}
