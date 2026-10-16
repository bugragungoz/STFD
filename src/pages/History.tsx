import { useEffect, useState } from 'preact/hooks';
import { api } from '../api';
import type { HistoryEntry } from '../api/types';
import { AppIcon } from '../components/AppIcon';
import { Button } from '../components/Button';
import { Icon } from '../components/Icon';
import { EmptyState, PageHeader } from '../components/Misc';
import { t } from '../i18n';
import { errorMessage } from '../lib/errors';
import { formatDateTime, formatSpan } from '../lib/format';
import { apps } from '../state/apps';
import { confirm } from '../state/dialogs';
import { history, loadHistory } from '../state/history';
import { toast } from '../state/toasts';

const OUTCOME_TONE: Record<HistoryEntry['outcome'], string> = { cleaned: 'badge-ok', nothing: '', kept: 'badge-info', notOpened: 'badge-warn' };

function Items({ icon, title, items, tone }: { icon: string; title: string; items: string[]; tone?: string }) {
  if (!items.length) return null;
  return (
    <div class="history-group">
      <h4 class="sub-title">{title}</h4>
      <ul class="history-items">
        {items.map((name, i) => (
          <li key={`${name}-${i}`}>
            <Icon name={icon} size={14} class={tone ?? 'muted'} />
            <span class="truncate">{name}</span>
          </li>
        ))}
      </ul>
    </div>
  );
}

export function HistoryRow({ entry, compact = false }: { entry: HistoryEntry; compact?: boolean }) {
  const [open, setOpen] = useState(false);
  const app = apps.value.find((a) => a.id === entry.appId);
  const chips = [
    entry.started.length ? t('history.started', { count: entry.started.length }) : null,
    entry.stopped.length ? t('history.stopped', { count: entry.stopped.length }) : null,
    entry.closed.length + entry.ended.length ? t('history.closed', { count: entry.closed.length + entry.ended.length }) : null,
  ].filter(Boolean) as string[];
  const hasDetails = entry.started.length + entry.stopped.length + entry.closed.length + entry.ended.length + entry.problems.length > 0;
  return (
    <li class="history-row">
      <div class="history-head">
        {compact ? null : <AppIcon path={app?.program ?? entry.appName} name={entry.appName} size={32} />}
        <div class="history-text">
          <span class="history-title">{compact ? formatDateTime(entry.endedAt) : entry.appName}</span>
          <span class="history-meta">
            {compact ? '' : `${formatDateTime(entry.endedAt)} · `}
            {t(`history.trigger.${entry.trigger}`)}
            {entry.openedAt ? ` · ${t('history.openFor', { time: formatSpan(entry.endedAt - entry.openedAt) })}` : ''}
          </span>
        </div>
        <span class="grow" />
        <span class={`badge ${OUTCOME_TONE[entry.outcome]}`}>{t(`history.outcome.${entry.outcome}`)}</span>
      </div>
      {chips.length || entry.problems.length ? (
        <div class="history-chips">
          {chips.map((c) => (
            <span key={c} class="badge">
              {c}
            </span>
          ))}
          {entry.problems.length ? (
            <span class="badge badge-warn">
              <Icon name="alert" size={12} />
              {t('history.problems', { count: entry.problems.length })}
            </span>
          ) : null}
          {hasDetails ? (
            <button type="button" class="link-button small" aria-expanded={open} onClick={() => setOpen(!open)}>
              {open ? t('history.hide') : t('history.show')}
            </button>
          ) : null}
        </div>
      ) : null}
      {open ? (
        <div class="history-details">
          <Items icon="play" title={t('history.startedList')} items={entry.started} />
          <Items icon="stop" title={t('history.stoppedList')} items={entry.stopped} />
          <Items icon="check" title={t('history.closedList')} items={entry.closed} tone="tone-ok" />
          <Items icon="close" title={t('history.endedList')} items={entry.ended} />
          {entry.problems.length ? (
            <div class="history-group">
              <h4 class="sub-title">{t('history.problemList')}</h4>
              <ul class="history-items">
                {entry.problems.map((p, i) => (
                  <li key={`${p.what}-${i}`}>
                    <Icon name="alert" size={14} class="tone-warn" />
                    <span>
                      <strong>{p.what}</strong>: {t(`problem.${p.code}`, { detail: p.detail })}
                    </span>
                  </li>
                ))}
              </ul>
            </div>
          ) : null}
        </div>
      ) : null}
    </li>
  );
}

export function History() {
  useEffect(() => {
    void loadHistory();
  }, []);
  const list = history.value;
  const clear = async () => {
    const ok = await confirm({ title: t('history.clearConfirm.title'), body: t('history.clearConfirm.body'), confirmLabel: t('history.clear'), cancelLabel: t('common.cancel'), danger: true });
    if (!ok) return;
    try {
      await api.clearHistory();
      await loadHistory();
    } catch (e) {
      toast(errorMessage(e), { tone: 'warn' });
    }
  };
  return (
    <section class="view">
      <div class="view-body">
        <PageHeader title={t('history.title')} subtitle={t('history.subtitle')}>
          {list.length ? (
            <Button variant="quiet" icon="trash" onClick={() => void clear()}>
              {t('history.clear')}
            </Button>
          ) : null}
        </PageHeader>
        {list.length ? (
          <ul class="history-list">
            {list.map((h) => (
              <HistoryRow key={h.id} entry={h} />
            ))}
          </ul>
        ) : (
          <EmptyState icon="history" title={t('history.empty.title')} body={t('history.empty.body')} />
        )}
      </div>
    </section>
  );
}
