import { DOMWrapper, mount, flushPromises } from "@vue/test-utils";
import { expect, it, vi } from "vitest";
import Home from "./Home.vue";
import { createTaskCenter, taskCenterKey } from "../task-center/controller";
import {
  taskDetail,
  taskPage,
  taskPort,
} from "../../../../tests/self-service/support";
const controller = () => createTaskCenter(null, null);
it("shows appended activity beyond six rows and keeps it visible after a polling refresh", async () => {
  const task = taskDetail;
  const first = Array.from({ length: 6 }, (_, i) => task(`task-${i}`));
  const recent = vi
    .fn()
    .mockResolvedValueOnce({ ...taskPage(first), next: "10" })
    .mockResolvedValueOnce({ ...taskPage([task("seventh")]), next: "5" })
    .mockResolvedValueOnce({ ...taskPage(first), next: "11" })
    .mockResolvedValueOnce({ ...taskPage([task("seventh")]), next: "4" });
  const activity = createTaskCenter({ ...taskPort(), list: recent }, null);
  const c = controller();
  const wrapper = mount(Home, {
    props: { controller: c, draft: "", connected: false },
    global: { provide: { [taskCenterKey as symbol]: activity } },
  });
  try {
    await activity.loadRecent();
    await flushPromises();
    expect(wrapper.findAll(".activity-row")).toHaveLength(6);
    const more = () =>
      wrapper.findAll("button").find((b) => b.text() === "继续读取动态")!;
    await more().trigger("click");
    await flushPromises();
    expect(wrapper.findAll(".activity-row")).toHaveLength(7);
    await wrapper.findAll(".activity-row")[6].trigger("click");
    expect(wrapper.emitted("task")).toEqual([["seventh"]]);
    await activity.loadRecent();
    await flushPromises();
    expect(wrapper.findAll(".activity-row")).toHaveLength(7);
    expect(activity.state.next).toBe("4");
    await wrapper
      .findAll("button")
      .find((b) => b.text() === "查看任务记录 ›")!
      .trigger("click");
    expect(wrapper.emitted("navigate")).toEqual([["tasks"]]);
  } finally {
    wrapper.unmount();
    activity.dispose();
    c.dispose();
  }
});
it("shows device values as plain text and exposes all addresses without an unavailable banner", async () => {
  const c = controller();
  const wrapper = mount(Home, {
    attachTo: document.body,
    props: {
      controller: c,
      draft: "",
      connected: false,
      device: {
        attributes: {
          computerName: "<img src=x>",
          ipAddresses: ["192.0.2.24", "2001:db8::24"],
          loginUser: "studio",
        },
      },
    },
  });
  expect(wrapper.get(".identity-name").text()).toBe("<img src=x>");
  expect(wrapper.find(".identity-name img").exists()).toBe(false);
  expect(wrapper.text()).toContain("192.0.2.24");
  expect(wrapper.text()).toContain("+1");
  expect(wrapper.find('[title="192.0.2.24、2001:db8::24"]').exists()).toBe(
    true,
  );
  expect(wrapper.text()).toContain("studio");
  await wrapper.get('[aria-label="查看所有 IP 地址"]').trigger("click");
  expect(new DOMWrapper(document.body).get('[role="dialog"]').text()).toContain(
    "2001:db8::24",
  );
  c.state.error = "提交结果未确认；请查询原请求。";
  c.state.uncertain = true;
  await wrapper.vm.$nextTick();
  expect(wrapper.text()).toContain("任务需要关注");
  expect(wrapper.text()).not.toContain("执行服务暂不可用");
  expect(wrapper.text()).not.toContain("设备信息暂不可用");
  await wrapper.setProps({ device: null });
  expect(wrapper.get(".computer-identity").text()).not.toContain("studio");
  expect(wrapper.get(".computer-identity").text()).not.toContain("192.0.2.24");
  wrapper.unmount();
  c.dispose();
});

it("distinguishes missing data, a complete empty software observation and partial loaded counts", async () => {
  const ports = fixtureLocalPorts();
  const collectedAt = Math.floor(Date.now() / 1000);
  const read = vi.fn(async () => ({
    query: { id: "software.installations", revision: "1".repeat(64) },
    coverage: ["4".repeat(64)],
    collectedAt,
    quality: "complete" as const,
    enumerationComplete: true,
    stale: false,
    items: [],
    next: null,
  }));
  const model = createSoftware(ports.data, { ...ports.software!, read });
  const w = mount(Home, {
    props: { controller: controller(), draft: "", connected: false },
    global: { provide: { [softwareSummaryKey as symbol]: model.summary } },
  });
  try {
    expect(w.text()).toContain("软件清单尚未读取");
    expect(w.find(".storage-track").exists()).toBe(false);
    await model.discover();
    await flushPromises();
    expect(w.text()).toContain("完整清单");
    expect(w.text()).toContain("当前加载 0 项");
    model.state.sources[0].page!.enumerationComplete = false;
    await flushPromises();
    expect(w.text()).toContain("部分或过期清单");
  } finally {
    w.unmount();
    model.dispose();
  }
});
import { fixtureLocalPorts } from "../../../../tests/assistant/local-ports";
import { createSoftware, softwareSummaryKey } from "../software/controller";
