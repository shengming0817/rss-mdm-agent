import { useState } from "react";
import { Alert, Button } from "antd";
import { SectionCard } from "@rss-mdm-agent/ui";
import type { DeviceAccessPort } from "../self-service/native";
import type { SetupTask } from "../self-service/types";

export function MacAccountSetup({ port }: { port: DeviceAccessPort }) {
  type MacFacts = Extract<
    Awaited<ReturnType<DeviceAccessPort>>,
    { kind: "macStatus" }
  >;
  const [facts, setFacts] = useState<MacFacts | null>(null);
  const [observed, setObserved] = useState(false);
  const label = (value: boolean | null) =>
    value == null ? "未知" : value ? "已确认" : "未就绪";
  const [tasks, setTasks] = useState<SetupTask[]>([]);
  const [message, setMessage] = useState("");
  const [busy, setBusy] = useState(false);

  async function run(task?: SetupTask) {
    setBusy(true);
    setMessage("");
    try {
      if (task) {
        const reply = await port({
          kind: "prepareMac",
          task: task.task,
          attempt: task.attempt,
        });
        if (reply.kind !== "prepared") throw Error();
        setTasks((items) => items.filter((item) => item !== task));
        setMessage("密码已提交给本机服务，账号结果由原操作回报。");
      } else {
        setTasks([]);
        const reply = await port({ kind: "tasks" });
        if (reply.kind !== "tasks") throw Error();
        const pending = reply.tasks.filter((item) => item.createName);
        setTasks(pending);
        if (!pending.length) setMessage("当前员工没有待创建的本地账号。");
      }
    } catch {
      setTasks([]);
      setMessage("操作未获确认，请刷新当前员工的任务。");
    } finally {
      setBusy(false);
    }
  }
  return (
    <SectionCard title="Mac 本地账号">
      <Button
        disabled={busy}
        onClick={async () => {
          setBusy(true);
          try {
            const reply = await port({ kind: "macStatus" });
            setFacts(reply.kind === "macStatus" ? reply : null);
          } catch {
            setFacts(null);
          } finally {
            setObserved(true);
            setBusy(false);
          }
        }}
      >
        读取平台单点登录状态
      </Button>
      {observed &&
        (facts ? (
          <div>
            <p>
              系统扩展 {label(facts.extensionInstalled)} · 本机配置{" "}
              {label(facts.nativeConfigurationMatches)} · 本机用户注册{" "}
              {label(facts.nativeUserRegistered)}
            </p>
            <p>
              服务端设备注册 {label(facts.deviceRegistered)} · 用户注册{" "}
              {label(facts.userRegistered)} · 登录许可 {label(facts.loginReady)}
            </p>
            <p>
              {facts.observedAt
                ? `本机扩展观察于 ${new Date(facts.observedAt).toLocaleString()}`
                : "暂无本机扩展观察时间"}
            </p>
          </div>
        ) : (
          <p>平台单点登录状态未知，可刷新本人绑定或重试。</p>
        ))}
      <Button disabled={busy} onClick={() => void run()}>
        刷新建号任务
      </Button>
      {tasks.map((task) => (
        <Button
          key={task.task + task.attempt}
          disabled={busy}
          onClick={() => void run(task)}
        >
          为 {task.createName} 设置密码
        </Button>
      ))}
      {message && <Alert type="info" title={message} />}
    </SectionCard>
  );
}
