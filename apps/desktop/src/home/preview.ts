// Supplied only by the development assembly; native device status is never inferred from samples.
export interface HomePreview {
  network: string;
  networkDetail: string;
  storageFree: string;
  storageDetail: string;
  battery: string;
  batteryDetail: string;
  activities: { title: string; description: string; time: string }[];
}
export const homePreviewKey = "rss-home-design-preview";
