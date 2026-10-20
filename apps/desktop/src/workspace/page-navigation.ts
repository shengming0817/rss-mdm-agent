// Page state is registered once with the workspace. Switching chrome never remounts pages.
export interface PageResource {
  id: string;
  title: string;
  description: string;
  version?: string;
  status?: string;
  source: string;
}
export interface PageNavigation {
  resources?: PageResource[];
  sections: { id: string; title: string; kind?: string; group?: string }[];
  section: string;
  path: { id: string; title: string }[];
  searchEntries?: { id: string; title: string }[];
  reset(): void;
  openSection(id: string): void;
  setQuery?(value: string): void;
}
