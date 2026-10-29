import { expect, it } from "vitest";
import { snapshot } from "valtio/vanilla";
import { createWorkspace } from "../state/workspace";
import { shellFixture } from "../../../../tests/desktop/ui-support";
import { pageResources } from "./page-projection";
import { pageContext } from "./page-context";
it("uses the recent task read clock without requiring a task-list page and invalidates failed reads", async () => {
  const f = shellFixture(),
    workspace = createWorkspace(
      await f.input.environment!.identity.read(),
      f.input,
    );
  try {
    await workspace.tasks.loadRecent();
    // A sibling controller tick is not the observation clock for a newly received task page.
    workspace.system.state.now = Date.now() - 1000;
    const resources = () =>
      pageResources(
        workspace,
        "home",
        "overview",
        snapshot(workspace.device.state),
        snapshot(workspace.system.state),
        snapshot(workspace.software.state),
        snapshot(workspace.tasks.state),
      );
    const rows = resources();
    expect(rows).toHaveLength(1);
    expect(rows[0].collectedAt).toBe(workspace.tasks.state.recentAsOf);
    expect(rows[0].validUntil).toBeGreaterThan(workspace.system.state.now);
    expect(rows[0].revision).toBe(
      JSON.stringify(workspace.tasks.recentItems()[0].revision),
    );
    expect(
      pageContext(
        "home",
        "主页",
        [],
        rows,
        workspace.key,
        workspace.system.state.now,
      ).stale,
    ).toBe(false);
    workspace.tasks.state.recentError = "读取失败，保留事实";
    expect(
      pageContext(
        "home",
        "主页",
        [],
        resources(),
        workspace.key,
        workspace.system.state.now,
      ).stale,
    ).toBe(true);
  } finally {
    workspace.dispose();
  }
});
it("does not attach an installation without a valid inventory observation as verified", async () => {
  const f = shellFixture(),
    workspace = createWorkspace(
      await f.input.environment!.identity.read(),
      f.input,
    );
  try {
    await workspace.software.discover();
    for (const source of workspace.software.state.sources)
      if (source.page) source.page.collectedAt = null;
    const rows = pageResources(
      workspace,
      "software",
      "installed",
      snapshot(workspace.device.state),
      snapshot(workspace.system.state),
      snapshot(workspace.software.state),
      snapshot(workspace.tasks.state),
    );
    expect(rows.length).toBeGreaterThan(0);
    expect(rows.every((row) => row.verified === false)).toBe(true);
  } finally {
    workspace.dispose();
  }
});
