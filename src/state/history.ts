/** What Lockstep did, newest first. */
import { signal } from '@preact/signals';
import { api } from '../api';
import type { HistoryEntry } from '../api/types';

export const history = signal<HistoryEntry[]>([]);

export async function loadHistory(): Promise<void> {
  history.value = await api.getHistory();
}
