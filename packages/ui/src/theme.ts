import { theme, type ThemeConfig } from "antd";

export type ColorMode = "light" | "dark";
export type ThemePreference = ColorMode | "system";
// ref: ant-design components/config-provider/index.tsx@6.6.8.
// Values follow the approved v4 prototype; Ant Design and product CSS share this owner.
export const palette = {
  light: {
    accent: "#5750be",
    accentHover: "#4640a5",
    tint: "#eeecfa",
    bg: "#f8f9fb",
    surface: "#ffffff",
    nav: "#f2f3f5",
    text: "#22262f",
    muted: "#737986",
    line: "#e7e9ef",
    hover: "#f1f2f6",
    green: "#288760",
    greenBg: "#ebf6ef",
    amber: "#946215",
    amberBg: "#fff5df",
    red: "#bc4748",
    redBg: "#fff0ef",
  },
  dark: {
    accent: "#aaa0ef",
    accentHover: "#bcb3fb",
    tint: "#302b46",
    bg: "#191b23",
    surface: "#222530",
    nav: "#1d2029",
    text: "#e6e7ed",
    muted: "#969bac",
    line: "#343846",
    hover: "#2c303e",
    green: "#7bcba3",
    greenBg: "#23392e",
    amber: "#e1b968",
    amberBg: "#3e3423",
    red: "#ea9393",
    redBg: "#3e292d",
  },
} as const;
export const fontFamily =
  '-apple-system,BlinkMacSystemFont,"Segoe UI","PingFang SC","Microsoft YaHei",sans-serif';
export function productTheme(
  mode: ColorMode,
  reducedMotion: boolean,
): ThemeConfig {
  const p = palette[mode];
  return {
    algorithm: mode === "dark" ? theme.darkAlgorithm : theme.defaultAlgorithm,
    token: {
      colorPrimary: p.accent,
      colorPrimaryHover: p.accentHover,
      colorPrimaryBg: p.tint,
      colorBgBase: p.surface,
      colorBgContainer: p.surface,
      colorBgElevated: p.surface,
      colorBgLayout: p.bg,
      colorText: p.text,
      colorTextSecondary: p.muted,
      colorBorder: p.line,
      colorBorderSecondary: p.line,
      colorSuccessBg: p.greenBg,
      colorWarningBg: p.amberBg,
      colorErrorBg: p.redBg,
      colorInfo: p.accent,
      colorInfoBg: p.tint,
      colorInfoBorder: p.line,
      colorSuccess: p.green,
      colorWarning: p.amber,
      colorError: p.red,
      fontFamily,
      fontSize: 13,
      controlHeight: 36,
      borderRadius: 7,
      borderRadiusLG: 12,
      motion: !reducedMotion,
    },
    components: {
      Card: { bodyPadding: 22, headerFontSize: 15 },
      Button: { primaryColor: mode === "dark" ? "#211b37" : "#ffffff" },
      Table: {
        headerBg: p.bg,
        headerColor: p.muted,
        cellPaddingBlock: 16,
        cellPaddingInline: 16,
        fontSize: 12,
      },
      Menu: {
        itemBg: "transparent",
        itemSelectedBg: p.tint,
        itemSelectedColor: p.accent,
        itemBorderRadius: 8,
      },
      Tabs: { horizontalItemGutter: 25 },
      Drawer: { footerPaddingBlock: 16, footerPaddingInline: 24 },
    },
  };
}
