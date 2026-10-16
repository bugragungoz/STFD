import { useEffect, useState } from 'preact/hooks';
import { api } from '../api';
import type { Draft, ExeFile, FolderInfo, FoundShortcut, ServiceInfo, Suggested } from '../api/types';
import { AppIcon } from '../components/AppIcon';
import { Button } from '../components/Button';
import { Icon } from '../components/Icon';
import { Note, PageHeader, SearchBox, Segmented, TextField } from '../components/Misc';
import { Checkbox, Switch } from '../components/Switch';
import { t } from '../i18n';
import { errorMessage } from '../lib/errors';
import { fileName, formatWait, relativeTo } from '../lib/format';
import { putApp } from '../state/apps';
import { settings } from '../state/settings';
import { toast } from '../state/toasts';
import { showApp, showPage } from '../state/ui';
import { AddServiceDialog } from './AppDetail';

type Step = 'folder' | 'program' | 'services' | 'finish';
const STEPS: Step[] = ['folder', 'program', 'services', 'finish'];
const WAITS = [0, 10, 30, 60, 120, 300];

function Stepper({ step }: { step: Step }) {
  const at = STEPS.indexOf(step);
  return (
    <ol class="stepper" aria-label={t('add.steps')}>
      {STEPS.map((s, i) => (
        <li key={s} class={i < at ? 'is-done' : i === at ? 'is-current' : ''} aria-current={i === at ? 'step' : undefined}>
          <span class="step-badge" aria-hidden="true">
            {i < at ? <Icon name="check" size={16} /> : i + 1}
          </span>
          <span>{t(`add.step.${s}`)}</span>
        </li>
      ))}
    </ol>
  );
}

function ProgramRow({ exe, folder, chosen, onPick }: { exe: ExeFile; folder: string; chosen: boolean; onPick: () => void }) {
  return (
    <li>
      <button type="button" role="radio" aria-checked={chosen} class="pick-row" onClick={onPick}>
        <AppIcon path={exe.path} name={exe.description ?? fileName(exe.path)} size={32} />
        <span class="pick-text">
          <strong class="truncate">{exe.description ?? fileName(exe.path)}</strong>
          <span class="muted mono small truncate">{relativeTo(exe.path, folder)}</span>
        </span>
        {exe.gui === false ? <span class="badge">{t('add.program.console')}</span> : null}
        <span class="radio-dot" aria-hidden="true" />
      </button>
    </li>
  );
}

export function AddApp() {
  const [step, setStep] = useState<Step>('folder');
  const [info, setInfo] = useState<FolderInfo | null>(null);
  const [scanning, setScanning] = useState(false);
  const [program, setProgram] = useState<string | null>(null);
  const [name, setName] = useState('');
  const [nameEdited, setNameEdited] = useState(false);
  const [args, setArgs] = useState('');
  const [query, setQuery] = useState('');
  const [showNoise, setShowNoise] = useState(false);
  const [extraFolders, setExtraFolders] = useState<string[]>([]);
  const [related, setRelated] = useState<Suggested[] | null>(null);
  const [company, setCompany] = useState<string | null>(null);
  const [picked, setPicked] = useState<ServiceInfo[]>([]);
  const [serviceError, setServiceError] = useState<string | null>(null);
  const [addingService, setAddingService] = useState(false);
  const [start, setStart] = useState(true);
  const [stop, setStop] = useState(true);
  const [manual, setManual] = useState(true);
  const [desktop, setDesktop] = useState(true);
  const [startMenu, setStartMenu] = useState(false);
  const [found, setFound] = useState<FoundShortcut[]>([]);
  const [replace, setReplace] = useState<string[]>([]);
  const [wait, setWait] = useState<number | null>(null);
  const [closePrograms, setClosePrograms] = useState(true);
  const [saving, setSaving] = useState(false);

  const folder = info?.folder ?? '';
  const folders = [folder, ...extraFolders].filter(Boolean);

  const chooseFolder = async () => {
    const path = await api.pickFolder(t('add.folder.pick'));
    if (!path) return;
    setScanning(true);
    try {
      const got = await api.inspectFolder(path);
      setInfo(got);
      setProgram(null);
      setExtraFolders([]);
      setRelated(null);
      const first = got.programs.find((p) => !p.noise);
      if (first && !got.problem) void chooseProgram(first.path, false);
    } catch (e) {
      toast(errorMessage(e), { tone: 'warn' });
    } finally {
      setScanning(false);
    }
  };

  const chooseProgram = async (path: string, edited = nameEdited) => {
    setProgram(path);
    setRelated(null);
    try {
      const p = await api.inspectProgram(path);
      if (!edited) setName(p.name);
      setCompany(p.company);
    } catch (e) {
      toast(errorMessage(e), { tone: 'warn' });
    }
  };

  const browseProgram = async () => {
    const path = await api.pickProgram(t('add.program.pick'), folder);
    if (path) await chooseProgram(path);
  };

  // The services are read when their step opens (a second or so on a real Windows).
  useEffect(() => {
    if (step !== 'services' || !program || related) return;
    setServiceError(null);
    api
      .listServices(folders, program)
      .then((c) => {
        setRelated(c.related);
        setCompany(c.company);
        setPicked(c.related);
        setManual(true);
      })
      .catch((e) => setServiceError(errorMessage(e)));
  }, [step, program, related]);

  useEffect(() => {
    if (step !== 'finish' || !program) return;
    api
      .findShortcuts(program)
      .then((f) => {
        setFound(f);
        setReplace(f.filter((x) => x.movable).map((x) => x.path));
      })
      .catch(() => setFound([]));
  }, [step, program]);

  const automatic = picked.filter((s) => s.startType === 'automatic');
  const needPermission = picked.some((s) => !s.controllable) || (manual && start && automatic.length > 0);

  const save = async () => {
    if (!program) return;
    setSaving(true);
    const draft: Draft = {
      name: name.trim(),
      program,
      arguments: args.trim(),
      folders,
      services: picked.map((s) => ({ name: s.name, display: s.display, start, stop })),
      manual: manual && start ? automatic.map((s) => s.name) : [],
      alsoClose: [],
      keep: [],
      waitSeconds: wait,
      servicesDelaySeconds: picked.length && start ? 3 : 0,
      closePrograms,
      desktop,
      startMenu,
      replace: desktop ? replace : [],
    };
    try {
      const out = await api.saveApp(draft);
      putApp(out.app);
      showApp(out.app.id);
      if (out.permission === 'declined') toast(t('add.savedNoPermission', { name: out.app.name }), { tone: 'warn' });
      else if (out.problems.length) toast(t('add.savedProblems', { name: out.app.name, count: out.problems.length }), { tone: 'warn' });
      else toast(desktop ? t('add.savedDesktop', { name: out.app.name }) : t('add.saved', { name: out.app.name }), { tone: 'ok' });
    } catch (e) {
      toast(errorMessage(e), { tone: 'warn' });
      setSaving(false);
    }
  };

  const canNext: Record<Step, boolean> = {
    folder: !!info && !info.problem && info.programs.length > 0,
    program: !!program && name.trim().length > 0,
    services: related !== null || !!serviceError,
    finish: true,
  };
  const at = STEPS.indexOf(step);
  const next = () => setStep(STEPS[at + 1]!);
  const back = () => (at === 0 ? showPage('apps') : setStep(STEPS[at - 1]!));

  const q = query.trim().toLowerCase();
  const programs = (info?.programs ?? []).filter((p) => !q || p.path.toLowerCase().includes(q) || (p.description ?? '').toLowerCase().includes(q));
  const main = programs.filter((p) => !p.noise);
  const noise = programs.filter((p) => p.noise);
  const listed = showNoise || q ? programs : main;
  const chosenOutside = program && !programs.some((p) => p.path === program) ? program : null;

  return (
    <section class="view">
      <div class="view-body">
        <PageHeader title={t('add.title')} subtitle={t('add.subtitle')} />
        <Stepper step={step} />

        {step === 'folder' ? (
          <section class="card wizard-card" aria-labelledby="step-title">
            <h2 class="card-title" id="step-title">
              {t('add.folder.title')}
            </h2>
            <p class="card-text">{t('add.folder.body')}</p>
            <div class="inline-form">
              <Button variant={info ? 'outline' : 'primary'} icon="folder" onClick={() => void chooseFolder()} disabled={scanning}>
                {info ? t('add.folder.change') : t('add.folder.choose')}
              </Button>
              {scanning ? <span class="muted small">{t('add.folder.scanning')}</span> : null}
            </div>
            {info ? <p class="path mono">{info.folder}</p> : null}
            {info?.problem ? (
              <Note tone="warn" icon="alert" title={t('add.folder.cannot')}>
                {t(`folder.problem.${info.problem.kind}`)}
              </Note>
            ) : null}
            {info && !info.problem ? (
              info.programs.length ? (
                <p class="tone-ok small row-icon">
                  <Icon name="checkCircle" size={16} />
                  {t('add.folder.found', { count: info.programs.length })}
                </p>
              ) : (
                <Note tone="warn" icon="alert" title={t('add.folder.noPrograms')}>
                  {t('add.folder.noProgramsBody')}
                </Note>
              )
            ) : null}
          </section>
        ) : null}

        {step === 'program' && info ? (
          <section class="card wizard-card" aria-labelledby="step-title">
            <h2 class="card-title" id="step-title">
              {t('add.program.title')}
            </h2>
            <p class="card-text">{t('add.program.body')}</p>
            <div class="toolbar">
              <SearchBox value={query} onInput={setQuery} placeholder={t('add.program.search')} />
              <span class="grow" />
              <Button size="sm" variant="quiet" icon="folder" onClick={() => void browseProgram()}>
                {t('add.program.browse')}
              </Button>
            </div>
            <ul class="pick-list" role="radiogroup" aria-label={t('add.program.title')}>
              {chosenOutside ? <ProgramRow exe={{ path: chosenOutside, size: 0, depth: 0, gui: true, description: name || null, noise: false }} folder={folder} chosen onPick={() => {}} /> : null}
              {listed.map((p) => (
                <ProgramRow key={p.path} exe={p} folder={folder} chosen={p.path === program} onPick={() => void chooseProgram(p.path)} />
              ))}
            </ul>
            {!q && noise.length ? (
              <button type="button" class="link-button small" onClick={() => setShowNoise(!showNoise)}>
                {showNoise ? t('add.program.hideNoise') : t('add.program.showNoise', { count: noise.length })}
              </button>
            ) : null}
            <div class="form-grid">
              <TextField
                id="app-name"
                label={t('add.program.name')}
                hint={t('add.program.nameHint')}
                value={name}
                onInput={(v) => {
                  setName(v);
                  setNameEdited(true);
                }}
              />
              <TextField id="app-args" label={t('add.program.args')} hint={t('add.program.argsHint')} value={args} onInput={setArgs} mono />
            </div>
          </section>
        ) : null}

        {step === 'services' && info && program ? (
          <section class="card wizard-card" aria-labelledby="step-title">
            <h2 class="card-title" id="step-title">
              {t('add.services.title')}
            </h2>
            <p class="card-text">{t('add.services.body', { name: name || fileName(program) })}</p>
            {info.vendorFolders.length ? (
              <div class="sub-section">
                <h3 class="sub-title">{t('add.services.vendor')}</h3>
                <p class="muted small">{t('add.services.vendorHint')}</p>
                {info.vendorFolders.map((v) => (
                  <Checkbox
                    key={v}
                    checked={extraFolders.includes(v)}
                    onChange={(on) => {
                      setExtraFolders(on ? [...extraFolders, v] : extraFolders.filter((x) => x !== v));
                      setRelated(null);
                    }}
                    label={v}
                  />
                ))}
              </div>
            ) : null}
            {serviceError ? <Note tone="warn" icon="alert" title={t('add.services.failed')}>{serviceError}</Note> : null}
            {!related && !serviceError ? <p class="muted">{t('add.services.loading')}</p> : null}
            {related ? (
              <>
                <div class="toolbar">
                  <span class="toolbar-count muted">{related.length ? t('add.services.found', { count: related.length, company: company ?? '' }) : t('add.services.none')}</span>
                  <span class="grow" />
                  <Button size="sm" icon="plus" onClick={() => setAddingService(true)}>
                    {t('app.services.add')}
                  </Button>
                </div>
                <ul class="pick-list">
                  {[...related, ...picked.filter((p) => !related.some((r) => r.name === p.name))].map((s) => {
                    const on = picked.some((p) => p.name === s.name);
                    const why = 'why' in s ? (s as Suggested).why : null;
                    return (
                      <li key={s.name} class="pick-row is-static">
                        <Checkbox checked={on} onChange={(v) => setPicked(v ? [...picked, s] : picked.filter((p) => p.name !== s.name))} label={t('add.services.pickNamed', { name: s.display })} hideLabel />
                        <span class="pick-text">
                          <strong class="truncate">{s.display}</strong>
                          <span class="muted small truncate">
                            <span class="mono">{s.name}</span>
                            {s.startType === 'automatic' ? ` · ${t('add.services.startsWithWindows')}` : ''}
                          </span>
                        </span>
                        {why ? <span class="badge">{t(`add.services.why.${why}`)}</span> : <span class="badge">{t('add.services.why.added')}</span>}
                        <span class={`badge ${s.state === 'running' ? 'badge-ok' : ''}`}>{t(`svc.${s.state}`)}</span>
                      </li>
                    );
                  })}
                </ul>
                {picked.length ? (
                  <div class="sub-section">
                    <Switch checked={start} onChange={setStart} label={t('add.services.start', { name: name || fileName(program) })} />
                    <Switch checked={stop} onChange={setStop} label={t('add.services.stop', { name: name || fileName(program) })} />
                    {automatic.length && start ? <Switch checked={manual} onChange={setManual} label={t('add.services.manual', { count: automatic.length })} hint={t('add.services.manualHint')} /> : null}
                  </div>
                ) : null}
              </>
            ) : null}
            {addingService ? (
              <AddServiceDialog
                app={{ folders, program, services: picked }}
                onClose={() => setAddingService(false)}
                onAdd={(s) => {
                  setAddingService(false);
                  setPicked([...picked, s]);
                }}
              />
            ) : null}
          </section>
        ) : null}

        {step === 'finish' && program ? (
          <section class="card wizard-card" aria-labelledby="step-title">
            <h2 class="card-title" id="step-title">
              {t('add.finish.title')}
            </h2>
            <div class="summary">
              <AppIcon path={program} name={name} size={44} />
              <div class="summary-text">
                <strong>{name}</strong>
                <span class="muted small">
                  {t('add.finish.services', { count: picked.length })} · {t('add.finish.folders', { count: folders.length })}
                </span>
              </div>
            </div>
            <div class="sub-section">
              <h3 class="sub-title">{t('add.finish.shortcuts')}</h3>
              <Checkbox checked={desktop} onChange={setDesktop} label={t('shortcut.desktop')} />
              <Checkbox checked={startMenu} onChange={setStartMenu} label={t('shortcut.startMenu')} />
              {desktop
                ? found
                    .filter((f) => f.movable)
                    .map((f) => (
                      <Checkbox
                        key={f.path}
                        checked={replace.includes(f.path)}
                        onChange={(on) => setReplace(on ? [...replace, f.path] : replace.filter((r) => r !== f.path))}
                        label={t('add.finish.replace', { file: fileName(f.path).replace(/\.lnk$/i, '') })}
                      />
                    ))
                : null}
              {found
                .filter((f) => !f.movable)
                .map((f) => (
                  <Note key={f.path} icon="info" title={t('add.finish.public', { file: fileName(f.path).replace(/\.lnk$/i, '') })}>
                    {t('add.finish.publicBody')}{' '}
                    <button type="button" class="link-button" onClick={() => void api.reveal(f.path)}>
                      {t('common.show')}
                    </button>
                  </Note>
                ))}
            </div>
            <div class="setting-row">
              <span class="setting-label">{t('app.timing.wait', { name })}</span>
              <Segmented<'default' | `${number}`>
                label={t('app.timing.wait', { name })}
                value={wait == null ? 'default' : `${wait}`}
                options={[{ value: 'default', label: t('app.timing.default', { time: formatWait(settings.value?.waitSeconds ?? 30) }) }, ...WAITS.map((w) => ({ value: `${w}` as `${number}`, label: formatWait(w) }))]}
                onChange={(v) => setWait(v === 'default' ? null : Number(v))}
              />
              <span class="setting-meta">{t('app.timing.waitHint')}</span>
            </div>
            <Switch checked={closePrograms} onChange={setClosePrograms} label={t('app.programs.closeAll')} hint={t('app.programs.closeAllHint')} />
            {picked.length && needPermission ? (
              <Note icon="shield" title={t('add.finish.permission')}>
                {t('add.finish.permissionBody')}
              </Note>
            ) : null}
          </section>
        ) : null}

        <div class="wizard-actions">
          <Button variant="quiet" icon={at === 0 ? undefined : 'back'} onClick={back} disabled={saving}>
            {at === 0 ? t('common.cancel') : t('add.back')}
          </Button>
          <span class="grow" />
          {step === 'finish' ? (
            <Button variant="primary" icon="check" onClick={() => void save()} disabled={saving || !program}>
              {saving ? t('add.saving') : t('add.save')}
            </Button>
          ) : (
            <Button variant="primary" onClick={next} disabled={!canNext[step]}>
              {t('add.next')}
              <Icon name="chevronRight" size={16} />
            </Button>
          )}
        </div>
      </div>
    </section>
  );
}
