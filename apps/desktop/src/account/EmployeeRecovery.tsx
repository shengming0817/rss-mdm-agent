import { useState } from "react";
import { Alert, Button } from "antd";
import { SectionCard } from "@rss-mdm-agent/ui";
import {
  nativeEmployeeChange,
  type DeviceAccessPort,
} from "../self-service/native";
import type {
  AccessBindingStatus,
  SelfServiceCommands,
} from "../self-service/types";

type Intent = SelfServiceCommands["employee_change"]["input"];

export function EmployeeRecovery({ port }: { port: DeviceAccessPort }) {
  const [bindings, setBindings] = useState<AccessBindingStatus[]>([]);
  const [pending, setPending] = useState<Intent | null>(null);
  const [message, setMessage] = useState("");
  const [busy, setBusy] = useState(false);

  async function refresh() {
    setBusy(true);
    try {
      const reply = await port({ kind: "ownBindings" });
      if (reply.kind !== "ownBindings") throw Error();
      setBindings(reply.bindings);
      setMessage(
        reply.bindings.length ? "" : "当前企业账号与本机账号没有可管理的绑定。",
      );
    } catch {
      setBindings([]);
      setMessage("未能确认设备注册和当前账号。请检查组织连接后重试。");
    } finally {
      setBusy(false);
    }
  }

  async function act(input: Intent) {
    setBusy(true);
    try {
      await nativeEmployeeChange(input);
      if (input.stage === "open") setPending(input);
      setMessage(
        input.stage === "open"
          ? "在验证窗口登录同一组织账号，然后返回此处选择身份验证。"
          : input.stage === "verify"
            ? "在验证窗口使用通行密钥或组织身份提供方完成验证，然后返回此处提交。"
            : "请查看验证窗口中的服务端结果，返回后刷新本人绑定。",
      );
    } catch {
      setMessage(
        "未获得确认。请检查当前账号、验证窗口和网络，使用原操作重试。",
      );
    } finally {
      setBusy(false);
    }
  }

  return (
    <SectionCard title="本人设备登录与恢复">
      <Button disabled={busy} onClick={() => void refresh()}>
        刷新本人绑定
      </Button>
      {bindings.map((b) => {
        const rss =
          b.account?.platform === "windows" &&
          b.account.authority.kind === "local" &&
          b.configuration.windows.kind === "rss_factor";
        const title =
          b.account?.platform === "macos"
            ? "macOS 平台单点登录"
            : rss
              ? "Windows RSS 登录因子"
              : "Windows 原生登录";
        const effect =
          b.appliedRevision === b.revision && b.effect?.state === "applied"
            ? "本机已回报应用"
            : b.effect?.state === "failed"
              ? "本机回报失败"
              : "等待本机确认";
        const open = (change: Intent["change"]) =>
          void act({
            binding: b.id,
            revision: b.revision,
            change,
            stage: "open",
          });
        return (
          <div key={b.id}>
            <h3>{title}</h3>
            <p>
              当前修订 {b.revision} · {effect}
            </p>
            {rss && (
              <p>
                换手机或遗失旧手机后，可使用已有通行密钥或组织身份提供方验证，再设置新的验证器。
              </p>
            )}
            {b.account?.platform === "macos" && (
              <p>
                Mac
                登录因子由平台单点登录管理。设备注册失效时，可在此验证并修复本人绑定。
              </p>
            )}
            <div className="rss-account-actions">
              {rss && (
                <Button disabled={busy} onClick={() => open("reset_factors")}>
                  更换或重置验证器
                </Button>
              )}
              <Button disabled={busy} onClick={() => open("repair_binding")}>
                修复本人注册
              </Button>
              <Button
                disabled={busy}
                onClick={() =>
                  void act({
                    binding: b.id,
                    revision: b.revision,
                    change: "repair_binding",
                    stage: "status",
                  })
                }
              >
                查询原操作
              </Button>
            </div>
          </div>
        );
      })}
      {pending && (
        <div className="rss-account-actions">
          <Button
            disabled={busy}
            onClick={() => void act({ ...pending, stage: "verify" })}
          >
            选择身份验证
          </Button>
          <Button
            disabled={busy}
            type="primary"
            onClick={() => void act({ ...pending, stage: "submit" })}
          >
            提交已验证的操作
          </Button>
          <Button
            disabled={busy}
            onClick={() => {
              setPending(null);
              setMessage("本次交互已结束。已提交的操作可刷新查询。");
            }}
          >
            结束交互
          </Button>
        </div>
      )}
      {message && <Alert type="info" title={message} />}
    </SectionCard>
  );
}
