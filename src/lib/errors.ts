/** Turns an error from the Rust side into a sentence for the person. */
import { toAppError } from '../api/invoke';
import { t } from '../i18n';

const KNOWN = new Set(['unsupported', 'invalid', 'not_found', 'folder_problem', 'save_failed', 'services_failed', 'settings_not_saved', 'history_failed', 'shortcut_failed']);

export function errorMessage(e: unknown): string {
  const err = toAppError(e);
  const code = KNOWN.has(err.code) ? err.code : 'internal';
  return t(`errors.${code}`, { detail: err.detail || err.message });
}
