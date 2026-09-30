import { mount } from "@vue/test-utils";
import { expect, it } from "vitest";
import { MessageComposer, MessageStream } from "../src";

it("renders assistant Markdown structurally and copies the original fenced code", async () => {
  const wrapper = mount(MessageStream, {
    props: {
      items: [
        {
          id: "m",
          kind: "assistant",
          stable: true,
          text: "# Result\n\n- one\n- two\n\n```sh\necho '<script>'\n```\n\n| A | B |\n|---|---|\n| 1 | 2 |",
        },
      ],
    },
  });
  expect(wrapper.findAll("li")).toHaveLength(2);
  expect(wrapper.find("table").exists()).toBe(true);
  expect(wrapper.find("script").exists()).toBe(false);
  await wrapper.get('[aria-label="复制代码"]').trigger("click");
  expect(wrapper.emitted("copy")).toEqual([["echo '<script>'\n"]]);
});

it("keeps links and images inert while preserving useful text", () => {
  const wrapper = mount(MessageStream, {
    props: {
      items: [
        {
          id: "m",
          kind: "assistant",
          stable: false,
          text: "[Docs](https://example.com) ![Diagram](https://example.com/image.png) <img src=x onerror=alert(1)>",
        },
      ],
    },
  });
  expect(wrapper.findAll("a,img,script,iframe")).toHaveLength(0);
  expect(wrapper.text()).toContain("https://example.com");
  expect(wrapper.text()).toContain("Diagram");
});

it("cannot submit a form during composition even without KeyboardEvent.isComposing", async () => {
  const wrapper = mount(MessageComposer, { props: { modelValue: "选字" } });
  await wrapper.get("textarea").trigger("compositionstart");
  await wrapper.get("textarea").trigger("keydown", { key: "Enter" });
  await wrapper.get("form").trigger("submit");
  expect(wrapper.emitted("submit")).toBeUndefined();
  await wrapper.get("textarea").trigger("compositionend");
  await wrapper.get("form").trigger("submit");
  expect(wrapper.emitted("submit")).toEqual([["选字"]]);
});

it("preserves ordered list numbering and an explicit fold choice across streaming updates", async () => {
  const wrapper = mount(MessageStream, {
    props: {
      items: [
        {
          id: "m",
          kind: "assistant",
          stable: false,
          text: "3. three\n4. four\n\n" + "long ".repeat(500),
        },
      ],
    },
  });
  expect(wrapper.get("ol").attributes("start")).toBe("3");
  await wrapper.get('[aria-expanded="true"]').trigger("click");
  await wrapper.setProps({
    items: [
      {
        id: "m",
        kind: "assistant",
        stable: true,
        text: "3. three\n4. four\n\n" + "long ".repeat(510),
      },
    ],
  });
  expect(wrapper.get("[aria-expanded]").attributes("aria-expanded")).toBe(
    "false",
  );
});
