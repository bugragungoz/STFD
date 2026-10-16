/**
 * The backend: the Rust commands inside the app, or a simulated Windows when the page runs in a
 * plain browser (`npm run dev:web`, the UI tests and the screenshots).
 */
import type { App, Draft, FolderInfo, FoundShortcut, HistoryEntry, Plan, ProgramInfo, RemoveOutcome, SaveOutcome, ServiceChoice, Settings, Snapshot } from './types';
import { call } from './invoke';
import { mockApi } from './mock';

export interface Api {
  readonly simulated: boolean;
  getState(): Promise<Snapshot>;
  getIcon(path: string): Promise<string | null>;
  getSettings(): Promise<Settings>;
  setSettings(settings: Settings): Promise<Settings>;
  getHistory(): Promise<HistoryEntry[]>;
  clearHistory(): Promise<void>;
  inspectFolder(folder: string): Promise<FolderInfo>;
  inspectProgram(path: string): Promise<ProgramInfo>;
  listServices(folders: string[], program: string): Promise<ServiceChoice>;
  findShortcuts(program: string): Promise<FoundShortcut[]>;
  saveApp(draft: Draft): Promise<SaveOutcome>;
  updateApp(app: App): Promise<App>;
  grantPermission(id: string): Promise<SaveOutcome>;
  removeApp(id: string): Promise<RemoveOutcome>;
  openApp(id: string): Promise<void>;
  previewCleanup(id: string): Promise<Plan>;
  /** null when the app was waiting and closes on the next step. */
  closeNow(id: string): Promise<HistoryEntry | null>;
  keepRunning(id: string): Promise<void>;
  makeShortcuts(id: string, desktop: boolean, startMenu: boolean): Promise<App>;
  /** A folder the person picks; null when they cancel. */
  pickFolder(title: string, start?: string): Promise<string | null>;
  pickProgram(title: string, start?: string): Promise<string | null>;
  openUrl(url: string): Promise<void>;
  reveal(path: string): Promise<void>;
}

const tauriApi: Api = {
  simulated: false,
  getState: () => call('get_state'),
  getIcon: (path) => call('get_icon', { path }),
  getSettings: () => call('get_settings'),
  setSettings: (settings) => call('set_settings', { settings }),
  getHistory: () => call('get_history'),
  clearHistory: () => call('clear_history'),
  inspectFolder: (folder) => call('inspect_folder', { folder }),
  inspectProgram: (path) => call('inspect_program', { path }),
  listServices: (folders, program) => call('list_services', { folders, program }),
  findShortcuts: (program) => call('find_shortcuts', { program }),
  saveApp: (draft) => call('save_app', { draft }),
  updateApp: (app) => call('update_app', { app }),
  grantPermission: (id) => call('grant_permission', { id }),
  removeApp: (id) => call('remove_app', { id }),
  openApp: (id) => call('open_app', { id }),
  previewCleanup: (id) => call('preview_cleanup', { id }),
  closeNow: (id) => call('close_now', { id }),
  keepRunning: (id) => call('keep_running', { id }),
  makeShortcuts: (id, desktop, startMenu) => call('make_shortcuts', { id, desktop, startMenu }),
  async pickFolder(title, start) {
    const { open } = await import('@tauri-apps/plugin-dialog');
    const picked = await open({ directory: true, multiple: false, title, defaultPath: start });
    return typeof picked === 'string' ? picked : null;
  },
  async pickProgram(title, start) {
    const { open } = await import('@tauri-apps/plugin-dialog');
    const picked = await open({ directory: false, multiple: false, title, defaultPath: start, filters: [{ name: 'Programs', extensions: ['exe'] }] });
    return typeof picked === 'string' ? picked : null;
  },
  async openUrl(url) {
    const { openUrl } = await import('@tauri-apps/plugin-opener');
    await openUrl(url);
  },
  async reveal(path) {
    const { revealItemInDir } = await import('@tauri-apps/plugin-opener');
    await revealItemInDir(path);
  },
};

export const inTauri = typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window;
export const api: Api = inTauri ? tauriApi : mockApi;
