import { useEffect, useState } from 'preact/hooks';
import { api } from '../api';
import type { App, HistoryEntry, Plan, ServiceInfo, SvcState } from '../api/types';
import { AppIcon } from '../components/AppIcon';
import { Button } from '../components/Button';
import { Dialog } from '../components/Dialog';
import { Icon } from '../components/Icon';
import { IconButton } from '../components/IconButton';
import { Note, SearchBox, Segmented } from '../components/Misc';
import { PhaseBadge } from '../components/PhaseBadge';
import { Checkbox, Switch } from '../components/Switch';
import { t } from '../i18n';
import { errorMessage } from '../lib/errors';
import { fileName, formatAgo, formatCountdown, formatDateTime, formatWait } from '../lib/format';
import { appById, dropApp, now, putApp, statusOf } from '../state/apps';
import { confirm } from '../state/dialogs';
import { history, loadHistory } from '../state/history';
import { settings } from '../state/settings';
import { toast } from '../state/toasts';
import { openId, showPage } from '../state/ui';
import { openApp } from './Apps';
import { HistoryRow } from './History';

const WAITS = [0, 10, 30, 60, 120, 300];
const DELAYS = [0, 3, 5, 10];

/** Saves a change to the app; the screen shows it at once and settles on what was stored. */
async function save(app: App, patch: Partial<App>): Promise<void> {
  const next = { ...app, ...patch };
  putApp(next);
  try {
    putApp(await api.updateApp(next));
  } catch (e) {
    putApp(app);
    toast(errorMessage(e), { tone: 'warn' });
  }
}

function inside(path: string, folder: string): boolean {
  const p = path.toLowerCase();
  const f = folder.toLowerCase().replace(/[\\/]+$/, '');
  return p === f || p.startsWith(f + '\\');
}

/** True when STFD closes this program after the app (the core's App::owns). */
export function closes(app: App, path: string): boolean {
  const same = (a: string) => a.toLowerCase() === path.toLowerCase();
  if (app.keep.some(same)) return false;
  return same(app.program) || app.folders.some((f) => inside(path, f)) || app.alsoClose.some(same);
}

function serviceState(state: SvcState | null | undefined) {
  if (!state) return <span class="badge badge-warn">{t('app.services.missing')}</span>;
  const up = state !== 'stopped' && state !== 'stopping';
  return <span class={`badge ${up ? 'badge-ok' : ''}`}>{t(`svc.${state}`)}</span>;
}

function StatusCard({ app }: { app: App }) {
  const status = statusOf(app.id);
  const phase = status?.phase;
  const running = status?.running ?? [];
  const servicesUp = (status?.services ?? []).filter((s) => s.state && s.state !== 'stopped' && s.state !== 'stopping').length;
  let text: string;
  if (!phase) {
    text = running.length || servicesUp ? t('app.status.idleLeftovers', { programs: running.length, services: servicesUp }) : t('app.status.idle');
  } else if (phase.kind === 'starting') text = t('app.status.starting', { name: app.name });
  else if (phase.kind === 'open') text = t('app.status.open', { time: formatAgo(status?.openedAt ?? phase.since, now.value) });
  else if (phase.kind === 'closing') text = t('app.status.closing', { name: app.name, time: formatCountdown(phase.at - now.value) });
  else text = t('app.status.cleaning');

  return (
    <section class="card" aria-labelledby="status-title">
      <div class="card-head">
        <h2 class="card-title" id="status-title">
          {t('app.status.title')}
        </h2>
        <PhaseBadge phase={phase} />
      </div>
      <p class="card-text">{text}</p>
      {phase?.kind === 'closing' ? (
        <div class="actions">
          <Button variant="primary" icon="stop" onClick={() => void api.closeNow(app.id).catch((e) => toast(errorMessage(e), { tone: 'warn' }))}>
            {t('app.closeNow')}
          </Button>
          <Button
            onClick={async () => {
              await api.keepRunning(app.id);
              toast(t('app.kept', { name: app.name }));
              void loadHistory();
            }}
          >
            {t('app.keepRunning')}
          </Button>
        </div>
      ) : null}
      {running.length ? (
        <div class="sub-section">
          <h3 class="sub-title">{t('app.running.title')}</h3>
          <ul class="plain-list">
            {running.map((r) => (
              <li key={r.pid} class="plain-row">
                <Icon name={r.windowed ? 'window' : 'services'} size={16} class="muted" />
                <span class="truncate">{r.name}</span>
                {r.windowed ? <span class="badge">{t('app.running.window')}</span> : null}
                <span class="grow" />
                <span class="muted mono small truncate path-tail">{r.path}</span>
              </li>
            ))}
          </ul>
        </div>
      ) : null}
    </section>
  );
}

export function AddServiceDialog({ app, onClose, onAdd }: { app: { folders: string[]; program: string; services: { name: string }[] }; onClose: () => void; onAdd: (s: ServiceInfo) => void }) {
  const [all, setAll] = useState<ServiceInfo[] | null>(null);
  const [query, setQuery] = useState('');
  const [error, setError] = useState<string | null>(null);
  useEffect(() => {
    api
      .listServices(app.folders, app.program)
      .then((c) => setAll(c.all))
      .catch((e) => setError(errorMessage(e)));
  }, []);
  const q = query.trim().toLowerCase();
  const shown = (all ?? []).filter((s) => !app.services.some((r) => r.name.toLowerCase() === s.name.toLowerCase())).filter((s) => !q || s.display.toLowerCase().includes(q) || s.name.toLowerCase().includes(q) || (s.company ?? '').toLowerCase().includes(q));
  return (
    <Dialog title={t('services.addTitle')} size="list" onClose={onClose}>
      <div class="dialog-stack">
        <SearchBox value={query} onInput={setQuery} placeholder={t('services.search')} />
        {error ? <p class="tone-warn">{error}</p> : null}
        {!all && !error ? <p class="muted">{t('services.loading')}</p> : null}
        <ul class="pick-list">
          {shown.slice(0, 200).map((s) => (
            <li key={s.name}>
              <button type="button" class="pick-row" onClick={() => onAdd(s)}>
                <span class="pick-text">
                  <strong class="truncate">{s.display}</strong>
                  <span class="muted small truncate">
                    <span class="mono">{s.name}</span>
                    {s.company ? ` · ${s.company}` : ''}
                  </span>
                </span>
                {serviceState(s.state)}
                <Icon name="plus" size={16} />
              </button>
            </li>
          ))}
        </ul>
      </div>
    </Dialog>
  );
}

function ServicesCard({ app }: { app: App }) {
  const status = statusOf(app.id);
  const [adding, setAdding] = useState(false);
  const [granting, setGranting] = useState(false);
  const needs = app.services.filter((r) => (r.start || r.stop) && status?.services.find((s) => s.name === r.name)?.controllable === false && status?.services.find((s) => s.name === r.name)?.state);
  const setRule = (name: string, patch: Partial<App['services'][number]>) =>
    void save(app, { services: app.services.map((r) => (r.name === name ? { ...r, ...patch } : r)) });

  const grant = async () => {
    setGranting(true);
    try {
      const out = await api.grantPermission(app.id);
      putApp(out.app);
      if (out.permission === 'declined') toast(t('permission.declined'), { tone: 'warn' });
      else if (out.permission === 'failed') toast(t('permission.failed', { count: Math.max(1, out.problems.length) }), { tone: 'warn' });
      else toast(t('permission.granted'), { tone: 'ok' });
    } catch (e) {
      toast(errorMessage(e), { tone: 'warn' });
    } finally {
      setGranting(false);
    }
  };

  return (
    <section class="card" aria-labelledby="services-title">
      <div class="card-head">
        <h2 class="card-title" id="services-title">
          {t('app.services.title')}
        </h2>
        <Button size="sm" icon="plus" onClick={() => setAdding(true)}>
          {t('app.services.add')}
        </Button>
      </div>
      <p class="card-text">{t('app.services.subtitle', { name: app.name })}</p>
      {needs.length ? (
        <Note tone="warn" icon="shield" title={t('permission.needed', { count: needs.length })}>
          {t('permission.why')}{' '}
          <button type="button" class="link-button" onClick={() => void grant()} disabled={granting}>
            {granting ? t('permission.asking') : t('permission.give')}
          </button>
        </Note>
      ) : null}
      {app.services.length ? (
        <ul class="rule-list">
          {app.services.map((r) => {
            const s = status?.services.find((x) => x.name === r.name);
            return (
              <li key={r.name} class="rule-row">
                <span class="rule-text">
                  <strong class="truncate">{r.display}</strong>
                  <span class="mono muted small truncate">{r.name}</span>
                </span>
                {serviceState(s ? s.state : 'stopped')}
                <span class="rule-checks">
                  <Checkbox checked={r.start} onChange={(v) => setRule(r.name, { start: v })} label={t('app.services.start')} />
                  <Checkbox checked={r.stop} onChange={(v) => setRule(r.name, { stop: v })} label={t('app.services.stop')} />
                </span>
                <IconButton
                  size="sm"
                  icon="trash"
                  label={t('app.services.remove', { name: r.display })}
                  onClick={() => void save(app, { services: app.services.filter((x) => x.name !== r.name) })}
                />
              </li>
            );
          })}
        </ul>
      ) : (
        <p class="muted small">{t('app.services.none')}</p>
      )}
      {adding ? (
        <AddServiceDialog
          app={app}
          onClose={() => setAdding(false)}
          onAdd={(s) => {
            setAdding(false);
            void save(app, { services: [...app.services, { name: s.name, display: s.display, start: true, stop: true, original: { startType: null, granted: false } }] });
          }}
        />
      ) : null}
    </section>
  );
}

function ProgramsCard({ app }: { app: App }) {
  const seen = app.seen.filter((p) => p.toLowerCase() !== app.program.toLowerCase());
  const toggle = (path: string, close: boolean) => {
    const same = (a: string) => a.toLowerCase() === path.toLowerCase();
    const inFolders = app.folders.some((f) => inside(path, f));
    const keep = app.keep.filter((k) => !same(k));
    const alsoClose = app.alsoClose.filter((k) => !same(k));
    if (close && !inFolders) alsoClose.push(path);
    if (!close && inFolders) keep.push(path);
    void save(app, { keep, alsoClose });
  };
  return (
    <section class="card" aria-labelledby="programs-title">
      <h2 class="card-title" id="programs-title">
        {t('app.programs.title')}
      </h2>
      <Switch checked={app.closePrograms} onChange={(v) => void save(app, { closePrograms: v })} label={t('app.programs.closeAll')} hint={t('app.programs.closeAllHint')} />
      <div class="sub-section">
        <h3 class="sub-title">{t('app.programs.seen', { name: app.name })}</h3>
        {seen.length ? (
          <ul class="plain-list">
            {seen.map((p) => (
              <li key={p} class="plain-row">
                <Checkbox checked={app.closePrograms && closes(app, p)} disabled={!app.closePrograms} onChange={(v) => toggle(p, v)} label={t('app.programs.closeNamed', { name: fileName(p) })} hideLabel />
                <span class="truncate">{fileName(p)}</span>
                <span class="grow" />
                <span class="muted mono small truncate path-tail">{p}</span>
              </li>
            ))}
          </ul>
        ) : (
          <p class="muted small">{t('app.programs.seenEmpty', { name: app.name })}</p>
        )}
      </div>
    </section>
  );
}

function FoldersCard({ app }: { app: App }) {
  const add = async () => {
    const picked = await api.pickFolder(t('add.folder.pick'), app.folders[0]);
    if (!picked) return;
    try {
      const info = await api.inspectFolder(picked);
      if (info.problem) {
        toast(t(`folder.problem.${info.problem.kind}`), { tone: 'warn' });
        return;
      }
      if (!app.folders.some((f) => f.toLowerCase() === info.folder.toLowerCase())) await save(app, { folders: [...app.folders, info.folder] });
    } catch (e) {
      toast(errorMessage(e), { tone: 'warn' });
    }
  };
  return (
    <section class="card" aria-labelledby="folders-title">
      <div class="card-head">
        <h2 class="card-title" id="folders-title">
          {t('app.folders.title')}
        </h2>
        <Button size="sm" icon="plus" onClick={() => void add()}>
          {t('app.folders.add')}
        </Button>
      </div>
      <p class="card-text">{t('app.folders.subtitle', { name: app.name })}</p>
      <ul class="plain-list">
        {app.folders.map((f) => (
          <li key={f} class="plain-row">
            <Icon name="folder" size={16} class="muted" />
            <span class="mono small truncate">{f}</span>
            <span class="grow" />
            <IconButton size="sm" icon="external" label={t('common.show')} onClick={() => void api.reveal(f)} />
            {app.folders.length > 1 ? <IconButton size="sm" icon="trash" label={t('app.folders.remove', { folder: f })} onClick={() => void save(app, { folders: app.folders.filter((x) => x !== f) })} /> : null}
          </li>
        ))}
      </ul>
    </section>
  );
}

function TimingCard({ app }: { app: App }) {
  const fallback = settings.value?.waitSeconds ?? 30;
  type W = 'default' | `${number}`;
  return (
    <section class="card" aria-labelledby="timing-title">
      <h2 class="card-title" id="timing-title">
        {t('app.timing.title')}
      </h2>
      <div class="setting-row">
        <span class="setting-label">{t('app.timing.wait', { name: app.name })}</span>
        <Segmented<W>
          label={t('app.timing.wait', { name: app.name })}
          value={app.waitSeconds == null ? 'default' : `${app.waitSeconds}`}
          options={[{ value: 'default', label: t('app.timing.default', { time: formatWait(fallback) }) }, ...WAITS.map((w) => ({ value: `${w}` as W, label: formatWait(w) }))]}
          onChange={(v) => void save(app, { waitSeconds: v === 'default' ? null : Number(v) })}
        />
        <span class="setting-meta">{t('app.timing.waitHint')}</span>
      </div>
      {app.services.some((s) => s.start) ? (
        <div class="setting-row">
          <span class="setting-label">{t('app.timing.delay')}</span>
          <Segmented<`${number}`>
            label={t('app.timing.delay')}
            value={`${app.servicesDelaySeconds}`}
            options={DELAYS.map((d) => ({ value: `${d}` as `${number}`, label: formatWait(d) }))}
            onChange={(v) => void save(app, { servicesDelaySeconds: Number(v) })}
          />
          <span class="setting-meta">{t('app.timing.delayHint')}</span>
        </div>
      ) : null}
    </section>
  );
}

function ShortcutsCard({ app }: { app: App }) {
  const onDesktop = app.shortcuts.made.some((m) => /\\desktop\\/i.test(m));
  const inStart = app.shortcuts.made.some((m) => /\\start menu\\/i.test(m));
  const [desktop, setDesktop] = useState(onDesktop || !inStart);
  const [startMenu, setStartMenu] = useState(inStart);
  const [busy, setBusy] = useState(false);
  const make = async () => {
    setBusy(true);
    try {
      putApp(await api.makeShortcuts(app.id, desktop, startMenu));
      toast(t('app.shortcuts.made'), { tone: 'ok' });
    } catch (e) {
      toast(errorMessage(e), { tone: 'warn' });
    } finally {
      setBusy(false);
    }
  };
  return (
    <section class="card" aria-labelledby="shortcuts-title">
      <h2 class="card-title" id="shortcuts-title">
        {t('app.shortcuts.title')}
      </h2>
      <p class="card-text">{t('app.shortcuts.subtitle', { name: app.name })}</p>
      {app.shortcuts.made.length ? (
        <ul class="plain-list">
          {app.shortcuts.made.map((m) => (
            <li key={m} class="plain-row">
              <Icon name="link" size={16} class="muted" />
              <span class="mono small truncate">{m}</span>
              <span class="grow" />
              <IconButton size="sm" icon="external" label={t('common.show')} onClick={() => void api.reveal(m)} />
            </li>
          ))}
        </ul>
      ) : (
        <p class="muted small">{t('app.shortcuts.none')}</p>
      )}
      <div class="inline-form">
        <Checkbox checked={desktop} onChange={setDesktop} label={t('shortcut.desktop')} />
        <Checkbox checked={startMenu} onChange={setStartMenu} label={t('shortcut.startMenu')} />
        <Button size="sm" icon="refresh" disabled={busy || (!desktop && !startMenu)} onClick={() => void make()}>
          {app.shortcuts.made.length ? t('app.shortcuts.remake') : t('app.shortcuts.make')}
        </Button>
      </div>
    </section>
  );
}

function CleanupDialog({ app, onClose }: { app: App; onClose: () => void }) {
  const [plan, setPlan] = useState<Plan | null>(null);
  const [busy, setBusy] = useState(false);
  useEffect(() => {
    api
      .previewCleanup(app.id)
      .then(setPlan)
      .catch((e) => {
        toast(errorMessage(e), { tone: 'warn' });
        onClose();
      });
  }, []);
  const status = statusOf(app.id);
  const display = (name: string) => app.services.find((s) => s.name === name)?.display ?? name;
  const run = async () => {
    setBusy(true);
    try {
      const done: HistoryEntry | null = await api.closeNow(app.id);
      if (done) toast(t('cleanup.done', { services: t('apps.services', { count: done.stopped.length }), programs: t('cleanup.programCount', { count: done.closed.length + done.ended.length }) }), { tone: done.problems.length ? 'warn' : 'ok' });
      void loadHistory();
      onClose();
    } catch (e) {
      toast(errorMessage(e), { tone: 'warn' });
      setBusy(false);
    }
  };
  const empty = plan && !plan.programs.length && !plan.services.length;
  return (
    <Dialog
      title={t('cleanup.title', { name: app.name })}
      size="list"
      busy={busy}
      onClose={onClose}
      actions={
        <>
          <Button onClick={onClose} disabled={busy}>
            {t('common.cancel')}
          </Button>
          {empty ? null : (
            <Button variant="primary" icon="stop" disabled={!plan || busy} onClick={() => void run()}>
              {busy ? t('cleanup.closing') : t('cleanup.confirm')}
            </Button>
          )}
        </>
      }
    >
      {!plan ? <p class="muted">{t('cleanup.reading')}</p> : null}
      {empty ? <p class="bg-dialog-text">{t('cleanup.nothing', { name: app.name })}</p> : null}
      {plan && !empty ? (
        <div class="review-groups">
          <p class="review-intro">{t('cleanup.intro', { seconds: settings.value?.politeSeconds ?? 10 })}</p>
          {plan.programs.length ? (
            <div class="review-group">
              <h3>{t('cleanup.programs', { count: plan.programs.length })}</h3>
              <ul class="review-list">
                {plan.programs.map((p) => (
                  <li key={p.pid} class="review-item">
                    <span class="truncate">{p.name}</span>
                    <span class="muted mono small truncate">{p.path}</span>
                  </li>
                ))}
              </ul>
            </div>
          ) : null}
          {plan.services.length ? (
            <div class="review-group">
              <h3>{t('cleanup.services', { count: plan.services.length })}</h3>
              <ul class="review-list">
                {plan.services.map((s) => (
                  <li key={s} class="review-item">
                    <span class="truncate">{display(s)}</span>
                    <span class="muted mono small">{s}</span>
                  </li>
                ))}
              </ul>
            </div>
          ) : null}
          {plan.skipped.length ? (
            <div class="review-group">
              <h3>{t('cleanup.skipped', { count: plan.skipped.length })}</h3>
              <ul class="review-list">
                {plan.skipped.map((s) => (
                  <li key={s.what} class="review-item">
                    <span class="truncate">{s.what}</span>
                    <span class="review-reason">{t(`skip.${s.reason.kind}`, { name: 'service' in s.reason ? s.reason.service : 'app' in s.reason ? s.reason.app : '' })}</span>
                  </li>
                ))}
              </ul>
            </div>
          ) : null}
        </div>
      ) : null}
      {status?.phase ? <p class="muted small">{t('cleanup.whileOpen')}</p> : null}
    </Dialog>
  );
}

export function AppDetail() {
  const app = appById(openId.value);
  const [cleaning, setCleaning] = useState(false);
  const [removing, setRemoving] = useState(false);
  useEffect(() => {
    void loadHistory();
  }, [openId.value]);
  if (!app) {
    return (
      <section class="view">
        <div class="view-body">
          <Button icon="back" variant="quiet" onClick={() => showPage('apps')}>
            {t('app.back')}
          </Button>
        </div>
      </section>
    );
  }
  const status = statusOf(app.id);
  const phase = status?.phase;
  const idle = !phase;
  const recent = history.value.filter((h) => h.appId === app.id).slice(0, 5);

  const remove = async () => {
    const ok = await confirm({
      title: t('app.removeConfirm.title', { name: app.name }),
      body: t('app.removeConfirm.body', { name: app.name }) + (app.services.some((s) => s.original.granted || s.original.startType) ? `\n\n${t('app.removeConfirm.services')}` : ''),
      confirmLabel: t('app.remove'),
      cancelLabel: t('common.cancel'),
      danger: true,
    });
    if (!ok) return;
    setRemoving(true);
    try {
      const out = await api.removeApp(app.id);
      dropApp(app.id);
      showPage('apps');
      toast(out.permission === 'declined' ? t('app.removedServicesLeft', { name: app.name }) : t('app.removed', { name: app.name }), { tone: out.permission === 'declined' || out.problems.length ? 'warn' : 'ok' });
    } catch (e) {
      toast(errorMessage(e), { tone: 'warn' });
    } finally {
      setRemoving(false);
    }
  };

  return (
    <section class="view">
      <div class="view-body">
        <div>
          <Button icon="back" variant="quiet" size="sm" onClick={() => showPage('apps')}>
            {t('app.back')}
          </Button>
        </div>
        <header class="detail-head">
          <AppIcon path={app.program} name={app.name} size={56} />
          <div class="detail-title">
            <h1 class="view-title">{app.name}</h1>
            <span class="mono muted small truncate">{app.program}</span>
          </div>
          <div class="page-actions">
            {idle || phase?.kind === 'closing' ? (
              <Button variant="primary" icon="play" onClick={() => void openApp(app)}>
                {t('app.open')}
              </Button>
            ) : null}
            {idle ? (
              <Button icon="stop" onClick={() => setCleaning(true)}>
                {t('app.closeLeftovers')}
              </Button>
            ) : null}
            <Button variant="danger" icon="trash" disabled={removing} onClick={() => void remove()}>
              {t('app.remove')}
            </Button>
          </div>
        </header>
        <StatusCard app={app} />
        <div class="two-col">
          <div class="col">
            <ServicesCard app={app} />
            <TimingCard app={app} />
          </div>
          <div class="col">
            <ProgramsCard app={app} />
            <FoldersCard app={app} />
            <ShortcutsCard app={app} />
          </div>
        </div>
        {recent.length ? (
          <section class="card" aria-labelledby="recent-title">
            <div class="card-head">
              <h2 class="card-title" id="recent-title">
                {t('app.history.title')}
              </h2>
              <Button size="sm" variant="quiet" onClick={() => showPage('history')}>
                {t('app.history.all')}
              </Button>
            </div>
            <ul class="history-list compact">
              {recent.map((h) => (
                <HistoryRow key={h.id} entry={h} compact />
              ))}
            </ul>
            <p class="muted small">{t('app.history.since', { date: formatDateTime(app.createdAt) })}</p>
          </section>
        ) : null}
        {cleaning ? <CleanupDialog app={app} onClose={() => setCleaning(false)} /> : null}
      </div>
    </section>
  );
}

