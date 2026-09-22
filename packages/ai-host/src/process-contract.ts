// @generated from native-process::Ready. Do not edit.

export type Scope =
  | {
      kind: "processGroup";
      root: number;
    }
  | {
      kind: "jobObject";
      name: string;
    };

export interface Ready {
  artifact: string;
  launchId: string;
  launcherPid: number;
  scope: Scope;
  version: number;
  workerPid: number;
}
