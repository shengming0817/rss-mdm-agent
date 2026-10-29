import { useState } from "react";
import { Alert, Button, Input, InputNumber, Select, Switch } from "antd";
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
          ["http", "https", "socks"].includes(field.key) && field.writable,
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
  const [draft, setDraft] = useState(initial),
    [expected, setExpected] = useState(revision),
    [dirty, setDirty] = useState(false),
    writable = (key: string) =>
      !disabled &&
      !!target.fields.find((field) => field.key === key && field.writable),
    value = (key: string) =>
      target.fields.find((field) => field.key === key)?.value;
  if (!draft) return null;
  const update = (patch: SettingsPatch) => {
      setDraft(patch);
      setDirty(true);
    },
    reset = () => {
      setDraft(initial());
      setExpected(revision);
      setDirty(false);
    };
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
                  setDraft(deepClone(entry.patch) as SettingsPatch);
                  setExpected(entry.expected ?? revision);
                  setDirty(true);
                  onEntryUsed?.();
                }
              }}
            >
              替换草稿为入口参数
            </Button>
          }
        />
      )}
      {expected !== revision && dirty && (
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
          <p className="rss-muted">
            只提交本次编辑的手动代理。自动配置、自动发现与其它协议分别显示原观察。
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
      <div className="rss-settings-actions">
        <small>{dirty ? "有尚未保存的修改" : "修改仅在保存后提交"}</small>
        <Button disabled={disabled} onClick={reset}>
          还原草稿
        </Button>
        <Button
          type="primary"
          disabled={disabled || !dirty}
          onClick={() => void owner.save(draft, expected)}
        >
          保存更改
        </Button>
      </div>
    </div>
  );
}
