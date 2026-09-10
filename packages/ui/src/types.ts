/** Display models only; these are not backend wire contracts. */
export interface MessageItem {
  id: string;
  kind: "assistant" | "user" | "reasoning";
  text: string;
}
export interface NavigationItem {
  id: string;
  label: string;
  disabled?: boolean;
}
export interface StatusItem {
  id: string;
  label: string;
  message: string;
  tone: "neutral" | "success" | "warning" | "danger";
}
