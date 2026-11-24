import { useState } from "react";
import { Alert, Button } from "antd";
import { SectionCard } from "@rss-mdm-agent/ui";
import type { DeviceAccessPort } from "../self-service/native";
import type { SetupTask } from "../self-service/types";

export function MacAccountSetup({ port }: { port: DeviceAccessPort }) {
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
