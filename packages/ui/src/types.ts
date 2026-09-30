/** Display models only; these are not backend wire contracts. */
export type MessageItem =
  | { id: string; kind: "user"; text: string }
  | { id: string; kind: "reasoning"; text: string }
  | { id: string; kind: "assistant"; text: string; stable: boolean };
export interface NavigationItem {
  icon?:
    | "assistant"
    | "home"
    | "software"
    | "tools"
    | "tasks"
    | "device"
    | "settings";
  badge?: number;
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
