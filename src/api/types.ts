/** What the Rust side sends and takes, field for field (camelCase). */

export type StartType = 'boot' | 'system' | 'automatic' | 'manual' | 'disabled';
export type SvcState = 'stopped' | 'starting' | 'stopping' | 'running' | 'paused';

export interface ServiceOriginal {
  startType?: StartType | null;
  granted: boolean;
}

export interface ServiceRule {
  name: string;
  display: string;
  start: boolean;
  stop: boolean;
  original: ServiceOriginal;
}

export interface App {
  id: string;
  name: string;
  program: string;
  arguments: string;
  folders: string[];
  services: ServiceRule[];
  alsoClose: string[];
  keep: string[];
  waitSeconds: number | null;
  servicesDelaySeconds: number;
  closePrograms: boolean;
  shortcuts: { made: string[]; moved: { from: string; to: string }[] };
  seen: string[];
  createdAt: number;
}

export type Phase =
  | { kind: 'starting'; since: number }
  | { kind: 'open'; since: number }
  | { kind: 'closing'; since: number; at: number }
  | { kind: 'cleaning' };

export interface Target {
  pid: number;
  name: string;
  path: string;
  windowed: boolean;
}

export interface ServiceStatus {
  name: string;
  state: SvcState | null;
  controllable: boolean;
}

export interface AppStatus {
  id: string;
  phase: Phase | null;
  openedAt: number | null;
  running: Target[];
  services: ServiceStatus[];
}

export interface Snapshot {
  supported: boolean;
  now: number;
  apps: App[];
  status: AppStatus[];
}

export interface Settings {
  waitSeconds: number;
  politeSeconds: number;
  notify: boolean;
  watchAlways: boolean;
  language: string;
  theme: 'system' | 'dark' | 'light';
}

export type Trigger = 'shortcut' | 'adopted' | 'manual';
export type Outcome = 'cleaned' | 'nothing' | 'kept' | 'notOpened';

export interface Problem {
  what: string;
  code: string;
  detail: string;
}

export interface HistoryEntry {
  id: string;
  appId: string;
  appName: string;
  trigger: Trigger;
  outcome: Outcome;
  openedAt: number | null;
  endedAt: number;
  started: string[];
  stopped: string[];
  closed: string[];
  ended: string[];
  problems: Problem[];
}

export type FolderProblemKind = 'notAbsolute' | 'driveRoot' | 'system' | 'tooBroad' | 'own';

export interface ExeFile {
  path: string;
  size: number;
  depth: number;
  gui: boolean | null;
  description: string | null;
  noise: boolean;
}

export interface FolderInfo {
  folder: string;
  problem: { kind: FolderProblemKind } | null;
  programs: ExeFile[];
  vendorFolders: string[];
}

export interface ProgramInfo {
  path: string;
  name: string;
  company: string | null;
  description: string | null;
}

export interface ServiceInfo {
  name: string;
  display: string;
  state: SvcState;
  startType: StartType;
  program: string | null;
  dll: string | null;
  company: string | null;
  controllable: boolean;
}

export interface Suggested extends ServiceInfo {
  why: 'inFolder' | 'samePublisher';
}

export interface ServiceChoice {
  company: string | null;
  related: Suggested[];
  all: ServiceInfo[];
}

export interface FoundShortcut {
  path: string;
  movable: boolean;
}

export interface Draft {
  id?: string | null;
  name: string;
  program: string;
  arguments: string;
  folders: string[];
  services: { name: string; display: string; start: boolean; stop: boolean }[];
  manual: string[];
  alsoClose: string[];
  keep: string[];
  waitSeconds: number | null;
  servicesDelaySeconds: number;
  closePrograms: boolean;
  desktop: boolean;
  startMenu: boolean;
  replace: string[];
}

export type Permission = 'notNeeded' | 'granted' | 'declined' | 'failed';

export interface SaveOutcome {
  app: App;
  permission: Permission;
  problems: Problem[];
}

export interface RemoveOutcome {
  permission: Permission;
  problems: Problem[];
}

export type Skip =
  | { kind: 'system' }
  | { kind: 'otherUser' }
  | { kind: 'serviceProcess'; service: string }
  | { kind: 'inUse'; app: string }
  | { kind: 'dependentRunning'; service: string };

export interface Plan {
  programs: Target[];
  services: string[];
  skipped: { what: string; reason: Skip }[];
}
