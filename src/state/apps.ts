/**
 * The apps and what the watcher saw last, read once a second while the window is open, a clock for
 * countdowns, and program icons as they arrive.
 */
import { computed, signal } from '@preact/signals';
import { api } from '../api';
import type { App, AppStatus, Snapshot } from '../api/types';
import { errorMessage } from '../lib/errors';

export const snapshot = signal<Snapshot | null>(null);
export const stateError = signal<string | null>(null);
/** Now, once a second, for countdowns. */
export const now = signal(Date.now());

export const apps = computed<App[]>(() => snapshot.value?.apps ?? []);

export function statusOf(id: string): AppStatus | undefined {
  return snapshot.value?.status.find((s) => s.id === id);
}

export function appById(id: string | null): App | undefined {
  return id ? apps.value.find((a) => a.id === id) : undefined;
}

export async function refresh(): Promise<void> {
  try {
    snapshot.value = await api.getState();
    stateError.value = null;
  } catch (e) {
    stateError.value = errorMessage(e);
  }
}

let started = false;
export function startPolling(): void {
  if (started) return;
  started = true;
  void refresh();
  setInterval(() => {
    now.value = Date.now();
    if (typeof document === 'undefined' || document.visibilityState !== 'hidden') void refresh();
  }, 1000);
}

/** Puts a saved app in place at once, before the next read confirms it. */
export function putApp(app: App): void {
  const s = snapshot.value;
  if (!s) return;
  const exists = s.apps.some((a) => a.id === app.id);
  snapshot.value = { ...s, apps: exists ? s.apps.map((a) => (a.id === app.id ? app : a)) : [...s.apps, app] };
}

export function dropApp(id: string): void {
  const s = snapshot.value;
  if (s) snapshot.value = { ...s, apps: s.apps.filter((a) => a.id !== id), status: s.status.filter((x) => x.id !== id) };
}

const icons = signal<Record<string, string | null>>({});
const asked = new Set<string>();

/** The program's icon as a data URL; undefined until it arrives, null when it has none. */
export function iconFor(path: string): string | null | undefined {
  const known = icons.value[path];
  if (known === undefined && !asked.has(path)) {
    asked.add(path);
    api
      .getIcon(path)
      .then((url) => (icons.value = { ...icons.value, [path]: url }))
      .catch(() => (icons.value = { ...icons.value, [path]: null }));
  }
  return known;
}
