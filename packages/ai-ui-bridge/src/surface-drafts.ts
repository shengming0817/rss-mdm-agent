import type {
  ActionRequest,
  SurfaceState,
} from "@rss-mdm-agent/ai-contract/wire";

/** Ephemeral input and original answer identity; never an approval or execution journal. */
export class SurfaceMemory {
  readonly fields = new Map<string, string>();
  pending: ActionRequest | undefined;
  revision = -1;
  notice = "";
  sending = false;
  retired = false;
  rejected = false;
  private version = 0;
  private listeners = new Set<() => void>();
  snapshot = () => this.version;
  subscribe = (listener: () => void) => {
    this.listeners.add(listener);
    return () => {
      this.listeners.delete(listener);
    };
  };
  changed() {
    this.version++;
    for (const listener of this.listeners) listener();
  }
  edit(key: string, value: string) {
    if (this.retired || this.fields.get(key) === value) return;
    this.fields.set(key, value);
    this.changed();
  }
  retain(keys: ReadonlySet<string>) {
    let removed = false;
    for (const key of this.fields.keys())
      if (!keys.has(key)) {
        this.fields.delete(key);
        removed = true;
      }
    if (removed) {
      this.notice = "问题表单已更新，不兼容的旧输入已清空。";
      this.changed();
    }
  }
  retire() {
    this.retired = true;
    this.fields.clear();
    this.pending = undefined;
    this.sending = false;
    this.changed();
  }
}
export interface NodeGate {
  snapshot(): number;
  subscribe(listener: () => void): () => void;
  canEdit(): boolean;
  reason(): string;
}
/** Application-owned across renderer remounts, fenced by the authenticated namespace. */
export class SurfaceDrafts {
  private entries = new Map<string, SurfaceMemory>();
  acquire(surface: SurfaceState) {
    const n = surface.namespace;
    const memory = this.form(
      JSON.stringify([
        n.authorityId,
        n.tenantId,
        n.principalId,
        n.sessionId,
        surface.surfaceInstanceId,
        surface.generation,
      ]),
    );
    if (surface.revision > memory.revision) {
      memory.revision = surface.revision;
      memory.changed();
    }
    return memory;
  }
  form(key: string) {
    let memory = this.entries.get(key);
    if (!memory) {
      memory = new SurfaceMemory();
      this.entries.set(key, memory);
    }
    return memory;
  }
  clear() {
    for (const memory of this.entries.values()) memory.retire();
    this.entries.clear();
  }
}
