export interface SoftwarePreviewItem {
  id: string;
  name: string;
  letter: string;
  color: string;
  version: string | null;
  latest: string;
  installed: boolean;
  size: string;
  managed?: boolean;
  description: string;
}
export type SoftwarePreviewAction = "install" | "update" | "uninstall";
export interface SoftwarePreview {
  items: SoftwarePreviewItem[];
  revision: number;
  apply(id: string, action: SoftwarePreviewAction, revision: number): boolean;
}
// Only the explicit development assembly provides sample state and simulated actions.
export const softwarePreviewKey = "rss-software-design-preview";
