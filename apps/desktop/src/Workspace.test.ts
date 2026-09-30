import { mount, flushPromises } from "@vue/test-utils";
import { it, expect, vi } from "vitest";
import Workspace from "./Workspace.vue";
import Assistant from "./assistant/Assistant.vue";
import SelfService from "./self-service/SelfService.vue";
import { resourceOffer, snapshot } from "../../../tests/self-service/support";
import { createHostSettings } from "./settings/controller";

it("requires explicit conversation selection, keeps one view and preserves staged context on close", async () => {
  const w = mount(Workspace, {
    attachTo: document.body,
    props: {
      ready: true,
      busy: false,
      page: "software",
      host: createHostSettings(undefined),
      selfServicePort: {
        snapshot: vi
          .fn()
          .mockResolvedValue({ ...snapshot(), available: [resourceOffer()] }),
        execute: vi.fn(),
        cancel: vi.fn(),
      },
    },
  });
  const resource = w.getComponent(SelfService);
  await flushPromises();
  await resource.get("summary").trigger("click");
  await resource.get('[data-action="ask-ai"]').trigger("click");
  await flushPromises();
  const drawer = w.get('dialog[aria-label="资源上下文 AI"]');
  expect(drawer.text()).toContain("办公套件");
  const c = w.getComponent(Assistant).props("controller");
  expect(c.context.value).toBeUndefined();
  expect(
    drawer.get('[data-action="choose-conversation"]').attributes("disabled"),
  ).toBeDefined();
  await drawer.get("select").setValue("new");
  await drawer.get('[data-action="choose-conversation"]').trigger("click");
  await flushPromises();
  expect(w.findAllComponents(Assistant)).toHaveLength(1);
  expect(c.context.value!.path).toEqual([
    "软件中心",
    "未提供分类",
    "办公套件",
    "软件详情",
  ]);
  c.draft.value = "我的问题";
  await drawer.trigger("cancel");
  await flushPromises();
  expect(w.find('dialog[aria-label="资源上下文 AI"]').exists()).toBe(false);
  expect(c.draft.value).toBe("我的问题");
  expect(c.context.value).toBeDefined();
  await w.setProps({ page: "assistant" });
  expect(w.getComponent(Assistant).props("controller")).toBe(c);
  w.unmount();
  expect(c.state.drafts.size).toBe(0);
});
