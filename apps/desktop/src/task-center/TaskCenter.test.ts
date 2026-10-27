import { mount, flushPromises } from "@vue/test-utils";
import { expect, it, vi } from "vitest";
import TaskCenter from "./TaskCenter.vue";
import { createTaskCenter } from "./controller";
import {
  taskPort,
  taskPage,
  taskDetail,
} from "../../../../tests/self-service/support";
it("opens the original task, shows independent evidence and returns to the stable list item", async () => {
  const port = taskPort(),
    item = taskDetail();
  item.nodes = [
    {
      id: "attempt/step",
      parent: null,
      stage: "running",
      bytes: 1024,
      total: null,
      attempt: "attempt",
      reason: null,
      exitCode: 0,
      detected: { kind: "absent" },
      quiescent: false,
    },
  ];
  port.list = vi.fn(async () => taskPage([item]));
  port.detail = vi.fn(async () => item);
  const c = createTaskCenter(port, null);
  await c.load();
  const wrapper = mount(TaskCenter, {
    props: { controller: c },
    attachTo: document.body,
  });
  await wrapper.get("button[data-task]").trigger("click");
  await flushPromises();
  expect(port.detail).toHaveBeenCalledWith(item.reference);
  expect(wrapper.get(".task-center-detail").text()).toContain(
    "进程退出码：0（不代表效果核实）",
  );
  expect(wrapper.get(".task-center-detail").text()).toContain(
    "1024 / 总量未知 字节",
  );
  await wrapper
    .findAll("button")
    .find((b) => b.text() === "返回任务列表")!
    .trigger("click");
  await flushPromises();
  expect(c.state.selected).toBeNull();
  expect(document.activeElement).toBe(wrapper.get("button[data-task]").element);
  wrapper.unmount();
  c.dispose();
});
it("keeps empty visible pages pageable and labels cached coverage without claiming online completeness", async () => {
  const port = taskPort();
  port.list = vi.fn(async () => ({
    ...taskPage(),
    next: "opaque-next",
    complete: false,
    lastSync: 1000,
    coverage: [{ source: "audit", complete: false, reasons: ["retention"] }],
  }));
  const c = createTaskCenter(port, null);
  await c.setQuery({ view: "management" });
  const wrapper = mount(TaskCenter, { props: { controller: c } });
  expect(wrapper.text()).toContain("本页无可见记录，仍可继续读取");
  expect(wrapper.text()).toContain("离线缓存");
  expect(wrapper.text()).toContain("部分或离线历史");
  expect(wrapper.findAll("button").some((b) => b.text() === "继续读取")).toBe(
    true,
  );
  wrapper.unmount();
  c.dispose();
});

it.each([
  [false, false],
  [false, true],
  [true, false],
  [true, true],
])(
  "keeps a closed detail closed after tab=%s and earlier read failure=%s",
  async (tab, failure) => {
    const item = taskDetail("closing-original");
    let finish!: (value: typeof item) => void;
    let reject!: (error: Error) => void;
    const pending = new Promise<typeof item>((resolve, fail) => {
      finish = resolve;
      reject = fail;
    });
    const port = {
      ...taskPort(),
      list: async () => taskPage([item]),
      detail: vi.fn().mockResolvedValueOnce(item).mockReturnValueOnce(pending),
      control: vi.fn(),
    };
    const c = createTaskCenter(port, null);
    await c.load();
    await c.openTask(item.reference);
    const wrapper = mount(TaskCenter, {
      props: { controller: c },
      attachTo: document.body,
    });
    try {
      const refreshing = c.openTask(item.reference);
      await wrapper.get(".task-center-detail > button").trigger("click");
      if (failure) reject(Error("late read failure"));
      else finish(item);
      expect(await refreshing).toBe("stale");
      await flushPromises();
      expect(wrapper.find(".task-center-detail").exists()).toBe(false);
      expect(c.state.selected).toBeNull();
      expect(c.state.error).toBe("");
      expect(port.control).not.toHaveBeenCalled();
    } finally {
      wrapper.unmount();
      c.dispose();
    }
  },
);
