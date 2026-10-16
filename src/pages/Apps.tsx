import { api } from '../api';
import type { App } from '../api/types';
import { AppIcon } from '../components/AppIcon';
import { Button } from '../components/Button';
import { Icon } from '../components/Icon';
import { Note, PageHeader } from '../components/Misc';
import { PhaseBadge } from '../components/PhaseBadge';
import { t } from '../i18n';
import { errorMessage } from '../lib/errors';
import { formatWait } from '../lib/format';
import { apps, snapshot, statusOf } from '../state/apps';
import { settings } from '../state/settings';
import { toast } from '../state/toasts';
import { showApp, showPage } from '../state/ui';

export async function openApp(app: App): Promise<void> {
  try {
    await api.openApp(app.id);
    toast(t('apps.opening', { name: app.name }), { tone: 'ok' });
  } catch (e) {
    toast(errorMessage(e), { tone: 'warn' });
  }
}

function AppCard({ app }: { app: App }) {
  const status = statusOf(app.id);
  const phase = status?.phase;
  const wait = app.waitSeconds ?? settings.value?.waitSeconds ?? 30;
  const parts = [t('apps.services', { count: app.services.length }), t('apps.wait', { time: formatWait(wait) })];
  const running = status?.running.length ?? 0;
  if (!phase && running > 0) parts.push(t('apps.leftovers', { count: running }));
  return (
    <li class="app-card">
      <button type="button" class="app-card-main" onClick={() => showApp(app.id)}>
        <AppIcon path={app.program} name={app.name} size={44} />
        <span class="app-card-text">
          <strong class="app-card-name truncate">{app.name}</strong>
          <span class="muted truncate">{parts.join(' · ')}</span>
        </span>
        <PhaseBadge phase={phase} />
        <Icon name="chevronRight" size={18} class="muted" />
      </button>
      {!phase || phase.kind === 'closing' ? (
        <div class="app-card-actions">
          <Button size="sm" icon="play" onClick={() => void openApp(app)} aria-label={t('apps.openNamed', { name: app.name })}>
            {t('apps.open')}
          </Button>
        </div>
      ) : null}
    </li>
  );
}

function HowItWorks() {
  return (
    <div class="how card">
      <h2 class="card-title">{t('apps.empty.title')}</h2>
      <ol class="how-steps">
        {(['folder', 'program', 'shortcut', 'close'] as const).map((step, i) => (
          <li key={step}>
            <span class="step-badge" aria-hidden="true">
              {i + 1}
            </span>
            <span>
              <strong>{t(`apps.empty.${step}.title`)}</strong>
              <span class="muted">{t(`apps.empty.${step}.body`)}</span>
            </span>
          </li>
        ))}
      </ol>
      <div>
        <Button variant="primary" icon="plus" onClick={() => showPage('add')}>
          {t('apps.addFirst')}
        </Button>
      </div>
    </div>
  );
}

export function Apps() {
  const snap = snapshot.value;
  const list = apps.value;
  return (
    <section class="view">
      <div class="view-body">
        <PageHeader title={t('apps.title')} subtitle={t('apps.subtitle')}>
          {list.length ? (
            <Button variant="primary" icon="plus" onClick={() => showPage('add')}>
              {t('apps.add')}
            </Button>
          ) : null}
        </PageHeader>
        {snap && !snap.supported ? <Note tone="warn" icon="alert" title={t('apps.unsupported.title')}>{t('apps.unsupported.body')}</Note> : null}
        {!snap ? null : list.length === 0 ? (
          <HowItWorks />
        ) : (
          <ul class="app-list" aria-label={t('apps.title')}>
            {list.map((a) => (
              <AppCard key={a.id} app={a} />
            ))}
          </ul>
        )}
      </div>
    </section>
  );
}
