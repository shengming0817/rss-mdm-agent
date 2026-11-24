import { useState } from "react";
import { Alert, Button, Form, Input } from "antd";
import { SectionCard } from "@rss-mdm-agent/ui";
import type { DeviceAccessPort } from "../self-service/native";
import type { SetupTask } from "../self-service/types";

export function WindowsLoginSetup({ port }: { port: DeviceAccessPort }) {
  const [tasks, setTasks] = useState<SetupTask[]>([]);
  const [setup, setSetup] = useState<{
    task: SetupTask;
    secret: string;
  } | null>(null);
  const [code, setCode] = useState("");
  const [password, setPassword] = useState("");
  const [busy, setBusy] = useState(false);
  const [message, setMessage] = useState("");
  function clear() {
    setSetup(null);
    setCode("");
    setPassword("");
  }
  async function run(action: () => Promise<void>) {
    setBusy(true);
    setMessage("");
    try {
      await action();
    } catch {
      setCode("");
      setPassword("");
      setMessage("未能确认操作。请重试当前验证码，或刷新任务。");
    } finally {
      setBusy(false);
    }
  }
  async function refresh() {
    clear();
    setTasks([]);
    const result = await port({ kind: "tasks" });
    if (result.kind !== "tasks") throw Error();
    setTasks(result.tasks);
    if (!result.tasks.length) setMessage("当前账号没有待设置的登录因子。");
  }
  async function prepare(task: SetupTask) {
    clear();
    const result = await port({ kind: "prepare", task: task.task });
    if (result.kind !== "setup" || result.task !== task.task) throw Error();
    setSetup({ task, secret: result.secret });
  }
  async function confirm() {
    if (!setup) return;
    const pending = port({
      kind: "confirmFactor",
      task: setup.task.task,
      code,
      password: setup.task.createName ? password : null,
    });
    setCode("");
    setPassword("");
    const result = await pending;
    if (result.kind !== "prepared") throw Error();
    clear();
    setTasks([]);
    setMessage("验证码已确认，等待本机应用和托管回执。");
  }
  return (
    <SectionCard title="Windows 登录因子">
      <Button disabled={busy} onClick={() => void run(refresh)}>
        读取待设置任务
      </Button>
      {tasks.map((task) => (
        <Button
          key={task.task}
          disabled={busy}
          onClick={() => void run(() => prepare(task))}
        >
          {task.createName
            ? `为新账号 ${task.createName} 设置验证器`
            : "为当前 Windows 账号设置验证器"}
        </Button>
      ))}
      {setup && (
        <Form layout="vertical" onFinish={() => void run(confirm)}>
          <Form.Item label="验证器设置密钥">
            <code>{setup.secret}</code>
          </Form.Item>
          <Form.Item label="六位动态码">
            <Input
              inputMode="numeric"
              autoComplete="one-time-code"
              maxLength={6}
              value={code}
              onChange={(e) => setCode(e.target.value)}
            />
          </Form.Item>
          {setup.task.createName && (
            <Form.Item label="新 Windows 账号密码">
              <Input.Password
                autoComplete="new-password"
                value={password}
                onChange={(e) => setPassword(e.target.value)}
              />
            </Form.Item>
          )}
          <Button
            htmlType="submit"
            type="primary"
            disabled={
              busy ||
              !/^\d{6}$/.test(code) ||
              (!!setup.task.createName && !password)
            }
          >
            确认
          </Button>
          <Button onClick={clear}>取消</Button>
        </Form>
      )}
      {message && <Alert type="info" title={message} />}
    </SectionCard>
  );
}
