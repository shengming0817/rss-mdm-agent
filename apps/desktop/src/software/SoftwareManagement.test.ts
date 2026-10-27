import { mount, flushPromises } from "@vue/test-utils";
import { expect, it, vi } from "vitest";
import SoftwareManagement from "./SoftwareManagement.vue";
import { createSoftware } from "./controller";
import { fixtureLocalPorts } from "../../../../tests/assistant/local-ports";
import { currentUser } from "../test-users";
import type { PageNavigation } from "../workspace/page-navigation";
it("prepares an external exact installation/action only after the original page click", async () => {
  const ports = fixtureLocalPorts(),
    model = createSoftware(ports.data, ports.software);
  const read = ports.software!.read;
  vi.spyOn(ports.software!, "read").mockImplementation(async (input) => {
    const page = await read(input);
    return {
      ...page,
      items: page.items.map((item) => ({
        ...item,
        actions: [
          {
            action: "uninstall" as const,
            available: true,
            reason: null,
            targetVersion: null,
          },
        ],
      })),
    };
  });
  const prepare = vi.spyOn(ports.software!, "prepare"),
    submit = vi.spyOn(ports.software!, "submit");
  const w = mount(SoftwareManagement, { props: { model, active: true } });
  try {
    await flushPromises();
    const item = model.installed.value.find((v) =>
      v.actions.some((a) => a.action === "uninstall" && a.available),
    )!;
    const nav = w.emitted("navigation")![0][0] as PageNavigation;
    expect(
      await nav.prepareEntry!({
        kind: "software",
        software: item.reference,
        action: "uninstall",
      }),
    ).toBe(true);
    await flushPromises();
    expect(prepare).not.toHaveBeenCalled();
    expect(submit).not.toHaveBeenCalled();
    expect(w.get('[aria-label="外部软件建议"]').text()).toContain(item.name);
    await w.get('[aria-label="外部软件建议"] button').trigger("click");
    await flushPromises();
    expect(prepare).toHaveBeenCalledWith(
      expect.objectContaining({
        software: item.reference,
        action: "uninstall",
      }),
    );
    expect(submit).not.toHaveBeenCalled();
    expect(
      await nav.prepareEntry!({
        kind: "software",
        software: {
          ...item.reference,
          query: { ...item.reference.query, revision: "f".repeat(64) },
        },
        action: "uninstall",
      }),
    ).toBe(false);
  } finally {
    w.unmount();
    model.dispose();
  }
});
it("uses the production model, retains its filtered list and ignores same-generation account refresh", async () => {
  const ports = fixtureLocalPorts(),
    model = createSoftware(ports.data, ports.software);
  const w = mount(SoftwareManagement, {
    attachTo: document.body,
    props: { model, active: true },
  });
  const old = currentUser.value;
  try {
    await flushPromises();
    const nav = w.emitted("navigation")![0][0] as PageNavigation;
    expect(nav.searchEntries).toHaveLength(4);
    await w.get('input[aria-label="搜索软件"]').setValue("Notes");
    await w.get('[data-software="notes"]').trigger("click");
    expect(w.get('[aria-label="软件详情"]').text()).toContain("Notes");
    await w.get(".software-detail-back").trigger("click");
    expect(
      (w.get('input[aria-label="搜索软件"]').element as HTMLInputElement).value,
    ).toBe("Notes");
    expect(document.activeElement).toBe(
      w.get('[data-software="notes"]').element,
    );
    currentUser.value = {
      kind: "userContext",
      schemaVersion: 1,
      generation: "refresh",
      user: {
        kind: "testUser",
        schemaVersion: 1,
        userId: "u",
        displayName: "U",
        nameKey: "u",
      },
      identity: {
        mode: "device",
        authorityId: "a",
        tenantId: "t",
        principalId: "u",
      },
    };
    await flushPromises();
    currentUser.value = structuredClone(currentUser.value);
    await flushPromises();
    await model.discover();
    expect(model.installed.value).toHaveLength(4);
    expect(await nav.openResource!("notes", "wrong-revision")).toBe(false);
  } finally {
    w.unmount();
    model.dispose();
    currentUser.value = old;
  }
});

it("reads only the visible software section and retains loaded inventory when hidden", async () => {
  const ports = fixtureLocalPorts();
  const discover = vi.spyOn(ports.data!, "query_directory");
  const read = vi.spyOn(ports.software!, "read");
  const history = vi.spyOn(ports.software!, "history");
  const model = createSoftware(ports.data, ports.software);
  const w = mount(SoftwareManagement, { props: { model, active: false } });
  try {
    await flushPromises();
    expect(discover).not.toHaveBeenCalled();
    expect(read).not.toHaveBeenCalled();
    expect(history).not.toHaveBeenCalled();
    await w.setProps({ active: true });
    await flushPromises();
    expect(discover).toHaveBeenCalledTimes(1);
    expect(read).toHaveBeenCalledTimes(1);
    expect(history).not.toHaveBeenCalled();
    await w.setProps({ active: false });
    await w.setProps({ active: true });
    await flushPromises();
    expect(discover).toHaveBeenCalledTimes(1);
    expect(read).toHaveBeenCalledTimes(1);
    await w
      .findAll("button")
      .find((button) => button.text() === "本地操作记录")!
      .trigger("click");
    await flushPromises();
    expect(history).toHaveBeenCalledTimes(1);
  } finally {
    w.unmount();
    model.dispose();
  }
});

it("stops queued inventory reads when hidden and resumes missing sources without rereading the first", async () => {
  const ports = fixtureLocalPorts();
  const directory = ports.data!.query_directory;
  const discover = vi
    .spyOn(ports.data!, "query_directory")
    .mockImplementation(async (input) => {
      const page = await directory(input);
      const original = page.items[0];
      return {
        ...page,
        items: [
          original,
          {
            ...original,
            reference: { ...original.reference, id: "software.extra" },
          },
        ],
      };
    });
  const originalRead = ports.software!.read;
  let release!: () => void;
  const paused = new Promise<void>((resolve) => {
    release = resolve;
  });
  const read = vi
    .spyOn(ports.software!, "read")
    .mockImplementation(async (input) => {
      if (input.read.selection.kind !== "query") throw Error("expected query");
      if (input.read.selection.query.id !== "software.extra") await paused;
      return {
        ...(await originalRead(input)),
        query: input.read.selection.query,
      };
    });
  const submit = vi.spyOn(ports.data!, "query_submit");
  const model = createSoftware(ports.data, ports.software);
  const w = mount(SoftwareManagement, { props: { model, active: true } });
  try {
    await flushPromises();
    expect(read).toHaveBeenCalledTimes(1);
    await w.setProps({ active: false });
    release();
    await flushPromises();
    expect(read).toHaveBeenCalledTimes(1);
    expect(model.state.sources[0].page).toBeDefined();
    await w.setProps({ active: true });
    await flushPromises();
    expect(discover).toHaveBeenCalledTimes(1);
    expect(read).toHaveBeenCalledTimes(2);
    expect(model.state.sources.every((source) => !!source.page)).toBe(true);
    expect(submit).not.toHaveBeenCalled();
  } finally {
    release();
    w.unmount();
    model.dispose();
  }
});
