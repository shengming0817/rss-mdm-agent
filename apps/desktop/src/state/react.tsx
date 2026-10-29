import {
  createContext,
  useContext,
  useEffect,
  useState,
  type ReactNode,
} from "react";
import { useSnapshot } from "valtio";
import { StatePanel, UiProvider } from "@rss-mdm-agent/ui";
import { desktopPlatform } from "../appearance";
import { createDesktop, type Desktop } from "./desktop";
import type { WorkspaceInput } from "./workspace";
const DesktopContext = createContext<{
  desktop: Desktop;
  platform: "macos" | "windows";
} | null>(null);
export function useDesktop() {
  const owner = useContext(DesktopContext);
  if (!owner) throw Error("desktop owner is required");
  return owner.desktop;
}
export function usePlatform() {
  const context = useContext(DesktopContext);
  if (!context) throw Error("desktop owner is required");
  return context.platform;
}
export function useWorkspace() {
  return useSnapshot(useDesktop().state).workspace;
}
function OwnedDesktop({
  owner,
  children,
  platform,
}: {
  platform: "macos" | "windows";
  owner: Desktop;
  children: ReactNode;
}) {
  const state = useSnapshot(owner.state),
    appearance = useSnapshot(owner.appearance.state);
  return (
    <DesktopContext.Provider value={{ desktop: owner, platform }}>
      <UiProvider
        theme={state.theme}
        platform={platform}
        reducedMotion={appearance.reducedMotion}
        highContrast={appearance.highContrast}
      >
        {children}
      </UiProvider>
    </DesktopContext.Provider>
  );
}
/** Effect-created owners survive ordinary renders and are recreated on actual remount. */
export function DesktopProvider({
  input,
  children,
  platform = desktopPlatform,
}: {
  platform?: "macos" | "windows";
  input?: WorkspaceInput;
  children: ReactNode;
}) {
  const [owner, setOwner] = useState<Desktop | null>(null);
  useEffect(() => {
    const created = createDesktop(input);
    setOwner(created);
    void created.start();
    return () => created.dispose();
  }, [input]);
  return owner ? (
    <OwnedDesktop owner={owner} platform={platform}>
      {children}
    </OwnedDesktop>
  ) : (
    <UiProvider>
      <StatePanel kind="loading" title="正在读取本机会话" />
    </UiProvider>
  );
}
