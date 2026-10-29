import { App, ConfigProvider } from "antd";
import zhCN from "antd/locale/zh_CN";
import {
  useEffect,
  useRef,
  useState,
  type CSSProperties,
  type ReactNode,
} from "react";
import {
  fontFamily,
  palette,
  productTheme,
  type ThemePreference,
} from "./theme";

function useMedia(query: string) {
  const [matches, setMatches] = useState(
    () => window.matchMedia(query).matches,
  );
  useEffect(() => {
    const media = window.matchMedia(query);
    const update = () => setMatches(media.matches);
    update();
    media.addEventListener("change", update);
    return () => media.removeEventListener("change", update);
  }, [query]);
  return matches;
}
export interface UiProviderProps {
  children: ReactNode;
  theme?: ThemePreference;
  reducedMotion?: boolean;
  highContrast?: boolean;
  platform?: "macos" | "windows";
}
export function UiProvider({
  children,
  theme = "system",
  reducedMotion = false,
  highContrast = false,
  platform,
}: UiProviderProps) {
  const dark = useMedia("(prefers-color-scheme: dark)");
  const reduce = useMedia("(prefers-reduced-motion: reduce)") || reducedMotion;
  const contrast = useMedia("(forced-colors: active)") || highContrast;
  const mode = theme === "system" ? (dark ? "dark" : "light") : theme;
  const container = useRef<HTMLDivElement>(null);
  const variables = Object.fromEntries(
    Object.entries(palette[mode]).map(([key, value]) => [
      `--${key.replace(/[A-Z]/g, (letter) => `-${letter.toLowerCase()}`)}`,
      value,
    ]),
  );
  return (
    <div
      ref={container}
      className="rss-ui"
      data-theme={mode}
      data-platform={platform}
      data-reduced-motion={reduce}
      data-high-contrast={contrast}
      style={{ ...variables, fontFamily, colorScheme: mode } as CSSProperties}
    >
      <ConfigProvider
        button={{ autoInsertSpace: false }}
        locale={zhCN}
        theme={productTheme(mode, reduce)}
        getPopupContainer={() => container.current!}
      >
        <App>{children}</App>
      </ConfigProvider>
    </div>
  );
}
