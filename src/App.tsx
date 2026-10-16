import { useEffect } from 'preact/hooks';
import { api } from './api';
import { formatNumber, t } from './i18n';
import { apps, snapshot, startPolling, stateError } from './state/apps';
import { loadHistory } from './state/history';
import { loadSettings } from './state/settings';
import { page, type Page } from './state/ui';
import { ConfirmHost } from './components/ConfirmHost';
import { Icon } from './components/Icon';
import { Snackbars } from './components/Snackbars';
import { AddApp } from './pages/AddApp';
import { AppDetail } from './pages/AppDetail';
import { Apps } from './pages/Apps';
import { History } from './pages/History';
import { REPO_URL, Settings } from './pages/Settings';

const NAV: { page: Page; icon: string; count?: () => number }[] = [
  { page: 'apps', icon: 'apps', count: () => apps.value.length },
  { page: 'history', icon: 'history' },
  { page: 'settings', icon: 'settings' },
];

/** The wordmark's mark: a program with the two things that come and go with it, in the accent ink. */
function Mark() {
  return (
    <svg class="wordmark-mark" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
      <rect x="3" y="3" width="12" height="6.5" rx="2" />
      <rect x="11" y="11.25" width="10" height="3.75" rx="1.4" />
      <rect x="11" y="17.25" width="10" height="3.75" rx="1.4" />
      <path d="M7 9.5v9.62h4M7 13.12h4" />
    </svg>
  );
}

function StatusBar() {
  const snap = snapshot.value;
  const followed = (snap?.status ?? []).filter((s) => s.phase).map((s) => snap?.apps.find((a) => a.id === s.id)?.name ?? '');
  let left: { icon: string; text: string; tone?: string };
  if (stateError.value) left = { icon: 'alert', text: stateError.value, tone: 'tone-warn' };
  else if (!snap) left = { icon: 'info', text: t('status.starting') };
  else if (followed.length) left = { icon: 'checkCircle', text: t('status.following', { names: followed.join(', ') }), tone: 'tone-ok' };
  else left = { icon: 'info', text: t('status.idle') };
  return (
    <footer class="statusbar">
      <span class={`status-item ${left.tone ?? ''}`}>
        <Icon name={left.icon} size={14} />
        <span class="truncate">{left.text}</span>
      </span>
      {api.simulated ? (
        <>
          <span class="sep" />
          <span class="status-item" title={t('status.demoHint')}>
            <Icon name="info" size={14} />
            {t('status.demo')}
          </span>
        </>
      ) : null}
      <span class="statusbar-gap" />
      <span class="status-item mono">{t('status.version', { version: __APP_VERSION__ })}</span>
      <button type="button" class="status-item" onClick={() => void api.openUrl(REPO_URL)} aria-label={t('status.github')} title={t('status.github')}>
        <Icon name="github" size={14} />
      </button>
    </footer>
  );
}

export function App() {
  useEffect(() => {
    void loadSettings();
    void loadHistory();
    startPolling();
  }, []);

  const current = page.value;
  const navPage = current === 'app' || current === 'add' ? 'apps' : current;
  return (
    <div class="app">
      <a class="skip-link" href="#main">
        {t('nav.skip')}
      </a>
      <nav class="sidebar" aria-label={t('nav.label')}>
        <div class="wordmark">
          <Mark />
          <span class="wordmark-text">Lockstep</span>
        </div>
        <div class="nav">
          {NAV.map((item) => {
            const count = item.count?.();
            return (
              <button key={item.page} type="button" class="nav-item" aria-current={navPage === item.page ? 'page' : undefined} onClick={() => (page.value = item.page)}>
                <Icon name={item.icon} size={19} />
                <span>{t(`nav.${item.page}`)}</span>
                {count ? <span class="nav-count">{formatNumber(count)}</span> : null}
              </button>
            );
          })}
        </div>
        <p class="sidebar-note">{t('nav.note')}</p>
      </nav>
      <main id="main" class="main" tabIndex={-1}>
        {current === 'apps' ? <Apps /> : null}
        {current === 'app' ? <AppDetail /> : null}
        {current === 'add' ? <AddApp /> : null}
        {current === 'history' ? <History /> : null}
        {current === 'settings' ? <Settings /> : null}
      </main>
      <StatusBar />
      <Snackbars />
      <ConfirmHost />
    </div>
  );
}
