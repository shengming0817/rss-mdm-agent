// Page state is registered once with the workspace. Switching chrome never remounts pages.
export interface PageResource {
  id: string;
  title: string;
  description: string;
  version?: string;
  status?: string;
  source: string;
  revision?: string;
  collectedAt?: number;
  validUntil?: number;
  verified?: boolean;
}
export interface PageNavigation {
  resources?: PageResource[];
  sections: { id: string; title: string; kind?: string; group?: string }[];
  section: string;
  path: { id: string; title: string }[];
  searchEntries?: { id: string; title: string; revision: string }[];
  openRequest?(request: string): Promise<void>;
  openResource?(id: string, revision: string): boolean | Promise<boolean>;
  /** Explicit stable external node owned by this page; never a DOM/title lookup. */
  openNode?(id: string): boolean | Promise<boolean>;
  search?: {
    loading: boolean;
    complete: boolean;
    hasMore: boolean;
    error: string;
    loadMore(): Promise<void>;
  };
  reset(): void;
  openSection(id: string): void;
  setQuery?(value: string): void;
}

/** Display navigation only; business admission always revalidates the original reference. */
export type NavigationTarget =
  | { kind: "section"; page: string; id: string }
  | { kind: "resource"; page: string; id: string; revision: string }
  | { kind: "request"; request: string };
export type SearchEntry = Exclude<NavigationTarget, { kind: "request" }> & {
  title: string;
  group: string;
};
