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

type Intent = NonNullable<SelfServiceCommands["employee_change"]["input"]>;

const labels = new Map(
  Object.entries({
    not_started: "等待执行",
    running: "执行中",
    succeeded: "执行成功",
    failed: "执行失败",
    waiting_reboot: "等待重启",
    unknown: "执行状态未知",
  }),
);
export function EmployeeRecovery({ port }: { port: DeviceAccessPort }) {
  const [application, setApplication] = useState(false);
  const [bindings, setBindings] = useState<AccessBindingStatus[]>([]);
  const [pending, setPending] = useState<Intent | null>(null);
  const [message, setMessage] = useState("");
  const [busy, setBusy] = useState(false);

  async function refresh() {
    setBusy(true);
    try {
      const stored = await nativeEmployeeChange(null);
      if (stored?.kind === "stored") setPending(stored.intent ?? null);
      const reply = await port({ kind: "ownBindings" });
      if (reply.kind !== "ownBindings") throw Error();
      setBindings(reply.bindings);
      setApplication(reply.applicationAvailable);
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
      const response = await nativeEmployeeChange(input);
      const receipt = response.kind === "progress" ? response.value : null;
      if (input.stage === "open") {
        setPending(input);
      }
      setMessage(
        input.stage === "open"
          ? "在验证窗口登录同一组织账号，然后返回此处选择身份验证。"
          : input.stage === "verify"
            ? "在验证窗口完成本人验证，然后返回此处提交。"
            : input.stage === "status" && receipt
              ? `原操作修订 ${receipt.revision} · ${receipt.received ? "设备已接收" : "等待设备接收"} · ${labels.get(receipt.execution) ?? "执行状态未知"} · ${receipt.effectObserved ? "已收到本机效果回报" : "尚无本机效果回报"}${receipt.cancellation === "confirmed" ? " · 已取消" : receipt.cancellation === "requested" ? " · 等待取消" : ""}${receipt.cleanupPending ? " · 等待清理" : ""}`
              : "服务端已受理本次操作。刷新本人绑定查看本机应用状态。",
      );
    } catch (e) {
      setMessage(
        e &&
          typeof e === "object" &&
          "code" in e &&
          e.code === "operation_absent"
          ? "服务端尚未登记这次操作。可重新打开验证窗口，继续同一次操作。"
          : "未获得确认。请检查当前账号、验证窗口和网络，使用原操作重试。",
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
            {b.account?.platform === "windows" && !rss && (
              <p>
                Windows Hello 请在 Windows 设置的“账号 →
                登录选项”管理。组织账号的验证方式可在身份提供方的账号安全页恢复。
              </p>
            )}
            {b.account?.platform === "macos" && (
              <p>
                Mac
                登录因子由平台单点登录管理。设备注册失效时，可在此验证并修复本人绑定。
              </p>
            )}
            <div className="rss-account-actions">
              {application && b.account?.platform === "macos" && (
                <Button
                  disabled={busy}
                  onClick={async () => {
                    setBusy(true);
                    try {
                      const r = await port({
                        kind: "openApplication",
                        binding: b.id,
                      });
                      if (r.kind !== "application") throw Error();
                      setMessage(
                        "已在 Safari 打开员工应用。应用会话由该应用确认。",
                      );
                    } catch {
                      setMessage(
                        "设备未批准本次应用登录，请检查平台单点登录注册和当前账号。",
                      );
                    } finally {
                      setBusy(false);
                    }
                  }}
                >
                  登录 RSS 员工自助 Web
                </Button>
              )}
              {rss && (
                <Button disabled={busy} onClick={() => open("reset_factors")}>
                  更换或重置验证器
                </Button>
              )}
              <Button disabled={busy} onClick={() => open("repair_binding")}>
                修复本人注册
              </Button>
            </div>
          </div>
        );
      })}
      {pending && (
        <div className="rss-account-actions">
          <p>
            {pending.change === "reset_factors" ? "重置验证器" : "修复本人注册"}{" "}
            · 发起时修订 {pending.revision}
          </p>
          <Button
            disabled={busy}
            onClick={() => void act({ ...pending, stage: "status" })}
          >
            查询原操作
          </Button>
          <Button
            disabled={busy}
            onClick={() => void act({ ...pending, stage: "open" })}
          >
            打开验证窗口
          </Button>
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
            onClick={async () => {
              setBusy(true);
              try {
                await nativeEmployeeChange({ ...pending, stage: "close" });
                setPending(null);
                setMessage(
                  "已清除本机保存的操作编号。已提交的操作仍可刷新绑定查看。",
                );
              } catch {
                setMessage("未能清除本次操作记录，请重试。");
              } finally {
                setBusy(false);
              }
            }}
          >
            清除本次操作记录
          </Button>
        </div>
      )}
      {message && <Alert type="info" title={message} />}
    </SectionCard>
  );
}
