import { reactive } from "vue";
import type { SoftwarePreview } from "../../apps/desktop/src/catalog-preview/software-preview";
// User-provided agent-a-preview.zip, index.html softwareSeed; UI-only reference data.
export function createSoftwarePreview(): SoftwarePreview {
  const state: SoftwarePreview = reactive({
    revision: 0,
    items: [
      {
        id: "workspace",
        name: "Workspace",
        letter: "W",
        color: "purple",
        version: "4.2",
        latest: "4.3",
        installed: true,
        size: "128 MB",
        description: "工作沟通与协作。",
      },
      {
        id: "notes",
        name: "Notes",
        letter: "N",
        color: "amber",
        version: "2.1",
        latest: "2.1",
        installed: true,
        size: "46 MB",
        description: "记录与整理笔记。",
      },
      {
        id: "browser",
        name: "Browser",
        letter: "B",
        color: "teal",
        version: "8.0",
        latest: "8.1",
        installed: true,
        size: "240 MB",
        description: "浏览网页与工作应用。",
      },
      {
        id: "sync",
        name: "Work Sync",
        letter: "S",
        color: "blue",
        version: "3.0",
        latest: "3.0",
        installed: true,
        size: "92 MB",
        managed: true,
        description: "组织提供的文件同步工具。",
      },
      {
        id: "pdf",
        name: "PDF Reader",
        letter: "P",
        color: "blue",
        version: null,
        latest: "5.1",
        installed: false,
        size: "72 MB",
        description: "查看与标注 PDF 文档。",
      },
      {
        id: "design",
        name: "Design Studio",
        letter: "D",
        color: "purple",
        version: null,
        latest: "1.4",
        installed: false,
        size: "320 MB",
        description: "图形与界面设计。",
      },
    ],
    apply(id, action, revision) {
      const item = state.items.find((item) => item.id === id);
      if (
        !item ||
        revision !== state.revision ||
        (action === "uninstall" && item.managed)
      )
        return false;
      if (
        (action === "install" && item.installed) ||
        (action !== "install" && !item.installed)
      )
        return false;
      item.installed = action !== "uninstall";
      item.version = item.installed ? item.latest : null;
      state.revision++;
      return true;
    },
  });
  return state;
}
