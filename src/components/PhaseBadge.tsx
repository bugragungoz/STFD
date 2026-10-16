import type { Phase } from '../api/types';
import { t } from '../i18n';
import { formatCountdown } from '../lib/format';
import { now } from '../state/apps';
import { Icon } from './Icon';

/** Where an app stands, in words with an icon (never color alone). */
export function PhaseBadge({ phase }: { phase: Phase | null | undefined }) {
  if (!phase) return <span class="badge">{t('phase.idle')}</span>;
  switch (phase.kind) {
    case 'starting':
      return (
        <span class="badge badge-info">
          <Icon name="refresh" size={12} />
          {t('phase.starting')}
        </span>
      );
    case 'open':
      return (
        <span class="badge badge-ok">
          <Icon name="play" size={12} />
          {t('phase.open')}
        </span>
      );
    case 'closing':
      return (
        <span class="badge badge-info">
          <Icon name="timer" size={12} />
          {t('phase.closing', { time: formatCountdown(phase.at - now.value) })}
        </span>
      );
    case 'cleaning':
      return (
        <span class="badge badge-info">
          <Icon name="refresh" size={12} />
          {t('phase.cleaning')}
        </span>
      );
  }
}
