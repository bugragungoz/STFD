/** Times and paths for people. */
import { formatNumber, locale, t } from '../i18n';

/** "just now", "4 min ago", "2 h ago", "3 days ago". */
export function formatAgo(at: number | null | undefined, now: number): string {
  if (!at) return t('time.never');
  const s = Math.max(0, Math.round((now - at) / 1000));
  if (s < 60) return t('time.now');
  if (s < 3600) return t('time.minutesAgo', { count: Math.floor(s / 60) });
  if (s < 86_400) return t('time.hoursAgo', { count: Math.floor(s / 3600) });
  return t('time.daysAgo', { count: Math.floor(s / 86_400) });
}

/** A date and time in the person's language: "16 Oct 2026, 15:42". */
export function formatDateTime(at: number): string {
  return new Intl.DateTimeFormat(locale.value, { dateStyle: 'medium', timeStyle: 'short' }).format(at);
}

/** A wait: "no wait", "30 s", "2 min", "1 min 30 s". */
export function formatWait(seconds: number): string {
  if (seconds <= 0) return t('time.noWait');
  const m = Math.floor(seconds / 60);
  const s = seconds % 60;
  if (m === 0) return t('time.seconds', { count: s });
  if (s === 0) return t('time.minutes', { count: m });
  return `${t('time.minutes', { count: m })} ${t('time.seconds', { count: s })}`;
}

/** A countdown: "0:25", "2:05". */
export function formatCountdown(ms: number): string {
  const total = Math.max(0, Math.ceil(ms / 1000));
  return `${Math.floor(total / 60)}:${String(total % 60).padStart(2, '0')}`;
}

/** How long something lasted: "2 h 5 min", "12 min", "40 s". */
export function formatSpan(ms: number): string {
  const s = Math.max(0, Math.round(ms / 1000));
  if (s < 60) return t('time.seconds', { count: s });
  const m = Math.round(s / 60);
  if (m < 60) return t('time.minutes', { count: m });
  const h = Math.floor(m / 60);
  const rest = m % 60;
  return rest ? `${t('time.hours', { count: h })} ${t('time.minutes', { count: rest })}` : t('time.hours', { count: h });
}

/** A path relative to a folder it is in: `Circuit Simulator 14\circuitsim.exe`. */
export function relativeTo(path: string, folder: string): string {
  const f = folder.replace(/[\\/]+$/, '');
  return path.toLowerCase().startsWith(f.toLowerCase() + '\\') ? path.slice(f.length + 1) : path;
}

export function fileName(path: string): string {
  return path.split(/[\\/]/).pop() ?? path;
}

export { formatNumber };
