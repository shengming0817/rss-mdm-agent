import assert from "node:assert/strict";
import { mkdirSync, writeFileSync } from "node:fs";
export async function checkTasksBrowser(page, fixture) {
  const directory = new URL("../.local-ci-runs/", import.meta.url);
  mkdirSync(directory, { recursive: true });
  const evidence = {
    assembly: "production React task pages with explicit fixture ports",
    platformProof: false,
    captures: [],
    checks: [],
  };
  const primary = (name) =>
    page
      .locator(".rss-hidden-primary")
      .getByRole("button", { name, exact: true })
      .click();
  const secondary = (name) =>
    page
      .locator(".rss-hidden-secondary")
      .getByRole("button", { name, exact: true })
      .click();
  const go = async (scene = "tasks") => {
    const reset = await fetch(new URL("/__fixture/scenario", fixture.url), {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({ scenario: "running" }),
    });
    assert.equal(reset.ok, true);
    await page.goto(`${fixture.url}?desktop&scene=${scene}`);
    await page
      .getByRole("button", { name: "切换侧栏布局", exact: true })
      .waitFor();
    await page.locator(".fixture-controls").evaluate((node) => {
      node.style.display = "none";
    });
    await primary("任务中心");
    await secondary("正在进行");
  };
  const capture = async (name) => {
    await page.evaluate(async () => {
      await document.fonts.ready;
      await new Promise((resolve) =>
        requestAnimationFrame(() => requestAnimationFrame(resolve)),
      );
    });
    const file = `react-tasks-${name}.png`;
    await page.screenshot({
      path: new URL(file, directory).pathname,
      animations: "disabled",
    });
    const geometry = await page.evaluate(() => ({
      scroll: document.documentElement.scrollWidth,
      viewport: innerWidth,
      rows: document.querySelectorAll(".rss-task-table tbody tr[data-row-key]")
        .length,
    }));
    assert.ok(
      geometry.scroll <= geometry.viewport + 1,
      `${name}: page must not overflow horizontally`,
    );
    evidence.captures.push({ file, ...geometry });
  };
  const open = async (request) => {
    await page
      .locator(".rss-task-table tr")
      .filter({ has: page.getByText(request, { exact: true }) })
      .getByRole("button", { name: "查看详情", exact: true })
      .click();
    const drawer = page.getByRole("dialog", { name: /任务详情/ });
    await drawer
      .getByRole("heading", { name: "处理过程", exact: true })
      .waitFor();
    return drawer;
  };
  const refresh = async () => {
    await page
      .getByRole("dialog", { name: /任务详情/ })
      .getByRole("button", { name: "查询原请求", exact: true })
      .click();
  };
  const advance = async (
    request,
    phase,
    assessment = null,
    observation = null,
  ) => {
    await page.evaluate(
      ({ request, phase, assessment, observation }) =>
        window.rssPageFixture.advanceTask(
          request,
          phase,
          assessment,
          observation,
        ),
      { request, phase, assessment, observation },
    );
    await refresh();
    await page.waitForFunction(
      ({ request, phase }) => {
        const task = window.rssDesktop.state.workspace.tasks.state.selected;
        return task?.reference.request === request && task.stage === phase;
      },
      { request, phase },
    );
  };
  await page.setViewportSize({ width: 1280, height: 900 });
  await go();
  await page.locator(".rss-task-table").waitFor();
  await capture("active-light-1280");
  let drawer = await open("fixture-ai-unknown");
  await drawer
    .getByText("设备效果未知，需要可信核对", { exact: true })
    .waitFor();
  const before = await page.evaluate(
    () =>
      window.rssPageFixture.events.filter((event) =>
        /policy_invoke|software_submit|task_cancel/.test(event.operation),
      ).length,
  );
  await refresh();
  await refresh();
  assert.equal(
    await page.evaluate(
      () =>
        window.rssPageFixture.events.filter((event) =>
          /policy_invoke|software_submit|task_cancel/.test(event.operation),
        ).length,
    ),
    before,
  );
  await capture("unknown-original");
  await page.keyboard.press("Escape");
  await drawer.waitFor({ state: "hidden" });
  assert.equal(
    await page.evaluate(() => document.activeElement?.dataset.taskReference),
    JSON.stringify({ kind: "request", request: "fixture-ai-unknown" }),
  );
  await secondary("历史记录");
  await page.getByText("已核实未达预期", { exact: true }).waitFor();
  await capture("history");
  drawer = await open("fixture-not-satisfied");
  assert.equal(
    await drawer.getByText("已核实符合预期", { exact: true }).count(),
    0,
  );
  await capture("verified-not-satisfied");
  await page.keyboard.press("Escape");
  await drawer.waitFor({ state: "hidden" });
  const source = page.getByRole("combobox", { name: "任务来源", exact: true });
  await source.focus();
  await source.press("ArrowDown");
  await source.press("ArrowDown");
  await source.press("ArrowDown");
  await source.press("Enter");
  await page.waitForFunction(
    () => window.rssDesktop.state.workspace.tasks.state.query.source === "ai",
  );
  await page
    .locator(".rss-task-table")
    .getByText("fixture-ai-unknown", { exact: true })
    .waitFor();
  assert.equal(
    await page.locator(".rss-task-table tbody tr[data-row-key]").count(),
    1,
  );
  await capture("source-ai");
  await source.focus();
  await source.press("ArrowDown");
  await source.press("ArrowUp");
  await source.press("ArrowUp");
  await source.press("Enter");
  await page.waitForFunction(
    () => window.rssDesktop.state.workspace.tasks.state.query.source === null,
  );
  await secondary("管理活动");
  await page.getByText("fixture-policy-activity", { exact: true }).waitFor();
  await capture("management");
  evidence.checks.push(
    "three original query views and source filter; honest page-scope summary/coverage; real node timeline; unknown reads never submit; verified/notSatisfied is not success; Escape restores the exact row trigger",
  );

  await go("tasks-control-unknown");
  drawer = await open("fixture-install");
  await drawer.getByRole("button", { name: "请求取消", exact: true }).click();
  const confirmation = page.getByRole("dialog", {
    name: "请求取消？",
    exact: true,
  });
  await confirmation
    .getByRole("button", { name: "请求取消", exact: true })
    .click();
  await confirmation.waitFor({ state: "hidden" });
  await drawer.getByText("原控制结果待核实", { exact: true }).waitFor();
  await capture("cancel-control-unknown");
  await drawer
    .getByRole("button", { name: "查询原控制请求", exact: true })
    .click();
  await page.waitForFunction(
    () => window.rssDesktop.state.workspace.tasks.state.pendingControl === null,
  );
  assert.equal(
    await page.evaluate(
      () => window.rssDesktop.state.workspace.tasks.state.selected.stage,
    ),
    "running",
  );
  await drawer.getByText("已请求取消", { exact: true }).waitFor();
  await capture("cancel-requested");
  assert.equal(
    await page.evaluate(
      () =>
        window.rssPageFixture.events.filter(
          (event) => event.operation === "task_cancel",
        ).length,
    ),
    1,
  );
  await advance("fixture-install", "cancelled", "noEffect");
  await drawer.getByText("可信证据确认取消且无效果", { exact: true }).waitFor();
  await capture("cancelled-confirmed");
  await page.keyboard.press("Escape");
  await drawer.waitFor({ state: "hidden" });
  evidence.checks.push(
    "original cancel CAS/control ID sent once; lost control receipt queried, acknowledged cancel stays running; only separate owner cancellation evidence ends the task; no rollback claim",
  );

  for (const intent of ["install", "update", "uninstall"]) {
    await go();
    if (intent === "uninstall") {
      await primary("软件中心");
      await secondary("已安装");
      await page
        .locator(".rss-installed-card")
        .filter({
          has: page.getByRole("button", { name: "Notes", exact: true }),
        })
        .getByRole("button", { name: "卸载", exact: true })
        .click();
      const prepare = page.getByRole("dialog");
      await prepare.getByRole("button", { name: "卸载", exact: true }).click();
      await page.waitForFunction(() =>
        window.rssPageFixture.events.some(
          (event) => event.operation === "software_submit",
        ),
      );
      await page.evaluate(async () => {
        const workspace = window.rssDesktop.state.workspace;
        const request = [...workspace.software.state.requests].at(-1);
        workspace.navigation.select("tasks", "history");
        await workspace.tasks.openTask({ kind: "request", request });
      });
    } else {
      await primary("软件中心");
      await secondary("发现");
      const title = intent === "install" ? "工作空间" : "创作画板";
      await page
        .locator(".rss-store-grid")
        .getByRole("button", { name: `查看 ${title}`, exact: true })
        .click();
      await page.getByRole("button", { name: "安装", exact: true }).click();
      await page
        .getByRole("button", { name: "查询进度", exact: true })
        .waitFor();
      await page.waitForFunction(
        () => !window.rssDesktop.state.workspace.policies.state.busy,
      );
      await capture(`${intent}-inline-progress`);
      await page.evaluate(async () => {
        const workspace = window.rssDesktop.state.workspace;
        const request = workspace.policies.state.submission.choice.request;
        workspace.navigation.select("tasks", "history");
        await workspace.tasks.openTask({ kind: "request", request });
      });
    }
    drawer = page.getByRole("dialog", { name: /任务详情/ });
    await drawer
      .getByRole("heading", { name: "处理过程", exact: true })
      .waitFor();
    const request = await page.evaluate(
      () =>
        window.rssDesktop.state.workspace.tasks.state.selected.reference
          .request,
    );
    if (intent === "update") {
      const frozen = await page.evaluate(() => {
        const tasks = window.rssDesktop.state.workspace.tasks.state.selected;
        const record =
          window.rssDesktop.state.workspace.policies.state.preparation;
        return {
          actual: tasks.softwareState,
          execution: tasks.facts.value.action.execution,
          resource: tasks.facts.value.action.operation.resource,
          task: record.action.task,
          offer: record.action.offer.presentation.summary,
        };
      });
      assert.deepEqual(frozen.actual, { kind: "present", version: "1.0" });
      assert.equal(frozen.execution.kind, "softwareProgram");
      assert.equal(frozen.execution.intent, "install");
      assert.equal(
        frozen.execution.steps.at(-1).package,
        frozen.task.resource.id,
      );
      assert.equal(frozen.execution.steps.at(-1).version, "2.0");
      assert.deepEqual(frozen.resource, {
        id: frozen.task.resource.id,
        revision: frozen.task.resource.version,
      });
      assert.equal(frozen.offer.steps.at(-1).version, "2.0");
    }
    await advance(request, "running");
    await advance(request, "executionEnded");
    assert.equal(
      await drawer.getByText("已核实符合预期", { exact: true }).count(),
      0,
    );
    if (intent === "update")
      assert.deepEqual(
        await page.evaluate(
          () =>
            window.rssDesktop.state.workspace.tasks.state.selected
              .softwareState,
        ),
        { kind: "present", version: "1.0" },
      );
    await capture(`${intent}-execution-ended`);
    await advance(
      request,
      "verified",
      "satisfied",
      intent === "uninstall"
        ? { kind: "absent" }
        : { kind: "present", version: intent === "update" ? "2.0" : "1.0" },
    );
    await drawer.getByText("已核实符合预期", { exact: true }).waitFor();
    await capture(`${intent}-verified`);
    const writes = await page.evaluate(() =>
      window.rssPageFixture.events.filter((event) =>
        ["policy_invoke", "software_submit"].includes(event.operation),
      ),
    );
    assert.equal(writes.length, 1);
    assert.equal(writes[0].request, request);
  }
  evidence.checks.push(
    "install and fixed-version update use original Policy preparation; native uninstall uses its original software preparation; each explicit write occurs once and executionEnded stays unverified until a separately supplied detector observation",
  );
  await go();
  await primary("软件中心");
  await secondary("已安装");
  await page
    .locator(".rss-installed-card")
    .filter({ has: page.getByRole("button", { name: "Notes", exact: true }) })
    .getByRole("button", { name: "卸载", exact: true })
    .click();
  const expired = page.getByRole("dialog");
  await expired.getByRole("button", { name: "卸载", exact: true }).waitFor();
  await page.evaluate(() => {
    const port = window.rssDesktop.state.workspace.software.port;
    const prepare = port.prepare.bind(port);
    port.prepare = async (input) => {
      const result = await prepare(input);
      result.startBeforeUnixMs = Date.now() - 1;
      window.rssPageFixture.operations.get(
        input.request,
      ).prepared.startBeforeUnixMs = result.startBeforeUnixMs;
      return result;
    };
  });
  await expired.getByRole("button", { name: "卸载", exact: true }).click();
  await page
    .getByText("本次操作已过期，未提交执行。", { exact: true })
    .waitFor();
  assert.equal(
    await page.evaluate(
      () =>
        window.rssPageFixture.events.filter(
          (event) => event.operation === "software_submit",
        ).length,
    ),
    0,
  );
  await capture("expired-frozen-preparation");
  evidence.checks.push(
    "expired original software preparation is shown in place and no submission is emitted",
  );
  for (const theme of ["light", "dark"])
    for (const width of [1280, 720, 420]) {
      await page.setViewportSize({ width, height: 900 });
      await go();
      await page.evaluate((theme) => window.rssDesktop.setTheme(theme), theme);
      await page.locator(".rss-task-table").waitFor();
      await capture(`active-${theme}-${width}`);
      drawer = await open("fixture-update");
      await capture(`detail-${theme}-${width}`);
      await page.keyboard.press("Escape");
      await drawer.waitFor({ state: "hidden" });
    }
  for (const scene of [
    "tasks-partial",
    "tasks-denied",
    "empty",
    "offline",
    "error",
    "readonly",
  ]) {
    await page.setViewportSize({ width: 1280, height: 900 });
    await go(scene);
    if (scene === "tasks-denied") {
      await page.locator(".rss-task-table").waitFor();
      await page
        .locator(".rss-task-table button[data-task-reference]")
        .first()
        .click();
      await page
        .getByText("原任务不可访问或状态未确认，请重试读取", { exact: true })
        .waitFor();
    } else if (["offline", "error"].includes(scene))
      await page
        .getByText("无法读取任务；已有记录待核实，请重读当前来源。", {
          exact: true,
        })
        .waitFor();
    else if (scene === "empty")
      await page.getByText("当前视图没有任务", { exact: true }).waitFor();
    else await page.locator(".rss-task-table").waitFor();
    if (scene === "readonly") {
      await page
        .locator(".rss-task-table button[data-task-reference]")
        .first()
        .click();
      drawer = page.getByRole("dialog", { name: /任务详情/ });
      await drawer
        .getByRole("heading", { name: "处理过程", exact: true })
        .waitFor();
      assert.equal(
        await drawer
          .getByRole("button", { name: "请求取消", exact: true })
          .count(),
        0,
      );
    }
    await capture(scene);
    assert.equal(
      await page.evaluate(
        () =>
          window.rssPageFixture.events.filter((event) =>
            /task_cancel|policy_confirm|software_submit/.test(event.operation),
          ).length,
      ),
      0,
    );
  }
  evidence.checks.push(
    "task list/detail 1280/720/420 light/dark; partial coverage, denied detail, empty, service error/offline and read-only perform no mutations; Windows native and real OS effects are separate",
  );
  writeFileSync(
    new URL("react-tasks-evidence.json", directory),
    JSON.stringify(evidence, null, 2),
  );
  console.log(
    "PASS React task center: original service views/control recovery, fixed preparations, independent verification, honest cancellation and v4 task page/detail visual matrix.",
  );
}
