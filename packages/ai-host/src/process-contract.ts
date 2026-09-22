// @generated from native-process::Ready and private-link-v1.json. Do not edit.

export type Scope =
  | {
      kind: "processGroup";
      root: number;
    }
  | {
      kind: "jobObject";
      name: string;
      session: number;
    };

export interface Ready {
  artifact: string;
  launchId: string;
  launcherPid: number;
  scope: Scope;
  version: number;
  workerPid: number;
}

export const privateLinkV1 = {
  version: 1,
  magic: [82, 83, 83],
  headerBytes: 9,
  maxFrameBytes: 524288,
  maxQueuedBytes: 1048576,
  maxQueuedFrames: 64,
  lanes: {
    native: ["native", "execution"],
    worker: ["control", "tools", "events"],
  },
} as const;
