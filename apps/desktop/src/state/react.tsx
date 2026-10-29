import {
  createContext,
  Fragment,
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
const DesktopContext = createContext<Desktop | null>(null);
export function useDesktop() {
  const owner = useContext(DesktopContext);
  if (!owner) throw Error("desktop owner is required");
  return owner;
}
export function useWorkspace() {
  return useSnapshot(useDesktop().state).workspace;
}
function OwnedDesktop({
  owner,
  children,
}: {
  owner: Desktop;
  children: ReactNode;
}) {
  const state = useSnapshot(owner.state),
    appearance = useSnapshot(owner.appearance.state);
  return (
    <DesktopContext.Provider value={owner}>
      <UiProvider
        theme={state.theme}
        platform={desktopPlatform}
        reducedMotion={appearance.reducedMotion}
        highContrast={appearance.highContrast}
      >
        <Fragment key={state.workspace?.key ?? "no-workspace"}>
          {children}
        </Fragment>
      </UiProvider>
    </DesktopContext.Provider>
  );
}
/** Effect-created owners survive ordinary renders and are recreated on actual remount. */
export function DesktopProvider({
  input,
  children,
}: {
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
    <OwnedDesktop owner={owner}>{children}</OwnedDesktop>
  ) : (
    <UiProvider>
      <StatePanel kind="loading" title="正在读取本机会话" />
    </UiProvider>
  );
}
