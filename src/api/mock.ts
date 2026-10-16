/**
 * A simulated Windows for the page in a plain browser: Multisim and Ansys Electronics Desktop with
 * the services the owner's old scripts started, programs that open and close on their own, and the
 * waits and cleanups the real app does. `?fast` shortens every wait (UI tests, screenshots);
 * `?scenario=empty` starts without apps, `?scenario=closing` with Multisim just closed.
 * Pickers return `window.__stfdPick` when a test set it.
 */
import type { Api } from './index';
import { AppError } from './invoke';
import type { App, AppStatus, Draft, ExeFile, FolderInfo, HistoryEntry, Phase, Plan, Problem, ServiceInfo, Settings, Suggested, SvcState, Target } from './types';

const params = new URLSearchParams(typeof location !== 'undefined' ? location.search : '');
const FAST = params.has('fast');
const SCENARIO = params.get('scenario') ?? '';
const ms = (n: number) => (FAST ? Math.min(n, 400) : n);

const NI = 'C:\\Program Files (x86)\\National Instruments';
const NI64 = 'C:\\Program Files\\National Instruments';
const MULTISIM = `${NI}\\Circuit Design Suite 14.3\\multisim.exe`;
const ANSYS_DIR = 'C:\\Program Files\\AnsysEM';
const ANSYS = `${ANSYS_DIR}\\v241\\Win64\\ansysedt.exe`;
const LICENSING = 'C:\\Program Files\\ANSYS Inc\\Shared Files\\Licensing\\winx64';

interface MockService extends ServiceInfo {
  /** The program that runs while it does, if it has its own. */
  pid?: number;
}

const svc = (name: string, display: string, program: string, company: string, state: SvcState = 'stopped', startType: ServiceInfo['startType'] = 'manual'): MockService => ({
  name,
  display,
  state,
  startType,
  program,
  dll: null,
  company,
  controllable: false,
});

const services: MockService[] = [
  svc('NIAuthenticationService', 'NI Authentication Service', `${NI}\\Shared\\Security\\niauth.exe`, 'National Instruments'),
  svc('NIDomainService', 'NI Domain Service', `${NI}\\Shared\\Security\\nidmsrv.exe`, 'National Instruments'),
  svc('nimDNSResponder', 'NI mDNS Responder Service', `${NI64}\\Shared\\mDNS Responder\\nimdnsResponder.exe`, 'National Instruments'),
  svc('NINetworkDiscovery', 'NI Network Discovery', `${NI}\\Shared\\Network Discovery\\nidiscovery.exe`, 'National Instruments', 'running', 'automatic'),
  svc('niPSPServiceLocator', 'NI PSP Service Locator', `${NI}\\Shared\\NI PSP\\lkads.exe`, 'National Instruments'),
  svc('NiSvcLoc', 'NI Service Locator', `${NI}\\Shared\\niSvcLoc\\nisvcloc.exe`, 'National Instruments', 'running', 'automatic'),
  svc('NITimeSync', 'NI Time Synchronization', `${NI}\\Shared\\Time Sync\\nitsync.exe`, 'National Instruments'),
  svc('ANSYSLicensingTomcat', 'ANSYS Licensing Tomcat', `${LICENSING}\\tomcat\\bin\\tomcat.exe`, 'ANSYS, Inc.', 'running', 'automatic'),
  svc('ANSYSLicenseManager', 'ANSYS, Inc. License Manager CVD', `${LICENSING}\\lmgrd.exe`, 'ANSYS, Inc.', 'running', 'automatic'),
  svc('Spooler', 'Print Spooler', 'C:\\Windows\\System32\\spoolsv.exe', 'Microsoft Corporation', 'running', 'automatic'),
  svc('wuauserv', 'Windows Update', 'C:\\Windows\\system32\\svchost.exe', 'Microsoft Corporation', 'running', 'manual'),
  svc('Steam Client Service', 'Steam Client Service', 'C:\\Program Files (x86)\\Common Files\\Steam\\steamservice.exe', 'Valve Corporation'),
  svc('AdobeARMservice', 'Adobe Acrobat Update Service', 'C:\\Program Files (x86)\\Common Files\\Adobe\\ARM\\1.0\\armsvc.exe', 'Adobe Inc.', 'running', 'automatic'),
];
const NI_SERVICES = services.filter((s) => s.company === 'National Instruments').map((s) => s.name);
const ANSYS_SERVICES = ['ANSYSLicensingTomcat', 'ANSYSLicenseManager'];

const rule = (name: string) => {
  const s = services.find((x) => x.name === name)!;
  return { name, display: s.display, start: true, stop: true, original: { startType: s.startType === 'automatic' ? ('automatic' as const) : null, granted: true } };
};

function baseApp(id: string, name: string, program: string, folders: string[], serviceNames: string[], extra: Partial<App> = {}): App {
  return {
    id,
    name,
    program,
    arguments: '',
    folders,
    services: serviceNames.map(rule),
    alsoClose: [],
    keep: [],
    waitSeconds: null,
    servicesDelaySeconds: 3,
    closePrograms: true,
    shortcuts: { made: [`C:\\Users\\Bugra\\Desktop\\${name}.lnk`], moved: [] },
    seen: [],
    createdAt: Date.now() - 86_400_000 * 3,
    ...extra,
  };
}

let apps: App[] =
  SCENARIO === 'empty'
    ? []
    : [
        baseApp('multisim', 'Multisim 14.3', MULTISIM, [NI, NI64], NI_SERVICES, {
          seen: [MULTISIM, `${NI}\\Shared\\niSvcLoc\\nisvcloc.exe`, `${NI}\\MAX\\NIMax.exe`, 'C:\\Users\\Bugra\\AppData\\Local\\NI\\crash\\nicrash.exe'],
          keep: [`${NI}\\MAX\\NIMax.exe`],
        }),
        baseApp('ansys', 'Ansys Electronics Desktop 2024 R1', ANSYS, [ANSYS_DIR, 'C:\\Program Files\\ANSYS Inc'], ANSYS_SERVICES, { waitSeconds: 60, servicesDelaySeconds: 5 }),
      ];
for (const a of apps) for (const r of a.services) services.find((s) => s.name === r.name)!.controllable = true;

let settings: Settings = { waitSeconds: 30, politeSeconds: 10, notify: true, watchAlways: false, language: 'system', theme: 'system' };
let history: HistoryEntry[] = [];
let nextPid = 4200;

interface Live {
  phase: Phase;
  openedAt: number | null;
  started: string[];
  problems: Problem[];
  /** Programs of the app running now. */
  running: Target[];
  /** When the simulated program closes on its own. */
  exitAt: number;
}
const sessions = new Map<string, Live>();

const file = (path: string) => path.split('\\').pop() ?? path;
const target = (path: string, windowed = false): Target => ({ pid: nextPid++, name: file(path), path, windowed });
const app = (id: string) => apps.find((a) => a.id === id);
const display = (name: string) => services.find((s) => s.name === name)?.display ?? name;

// Ansys left its license server running from an earlier session: "Close leftovers now" has work.
const leftovers = new Map<string, Target[]>([['ansys', [target(`${LICENSING}\\ansysli_server.exe`)]]]);

function entry(a: App, live: Live | undefined, outcome: HistoryEntry['outcome'], fields: Partial<HistoryEntry>): HistoryEntry {
  const e: HistoryEntry = {
    id: Math.random().toString(16).slice(2, 14),
    appId: a.id,
    appName: a.name,
    trigger: live ? 'shortcut' : 'manual',
    outcome,
    openedAt: live?.openedAt ?? null,
    endedAt: Date.now(),
    started: live?.started ?? [],
    stopped: [],
    closed: [],
    ended: [],
    problems: live?.problems ?? [],
    ...fields,
  };
  history = [e, ...history].slice(0, 300);
  return e;
}

function planFor(a: App): Plan {
  const live = sessions.get(a.id);
  const programs = (live?.running ?? leftovers.get(a.id) ?? []).filter((t) => a.closePrograms && !a.keep.includes(t.path));
  const stop = a.services.filter((r) => r.stop && services.find((s) => s.name === r.name)?.state === 'running').map((r) => r.name);
  return { programs, services: stop, skipped: [] };
}

function cleanUp(a: App): HistoryEntry {
  const live = sessions.get(a.id);
  const plan = planFor(a);
  for (const name of plan.services) services.find((s) => s.name === name)!.state = 'stopped';
  const closed = plan.programs.filter((t) => t.windowed || t.name !== 'ansysli_server.exe').map((t) => t.name);
  const ended = plan.programs.filter((t) => !t.windowed && t.name === 'ansysli_server.exe').map((t) => t.name);
  leftovers.delete(a.id);
  sessions.delete(a.id);
  const outcome = plan.programs.length || plan.services.length ? 'cleaned' : 'nothing';
  return entry(a, live, outcome, { stopped: plan.services.map(display), closed, ended, trigger: live ? 'shortcut' : 'manual' });
}

// One step of the simulated Windows: programs close on their own, waits run out, cleanups finish.
function tick() {
  const now = Date.now();
  for (const [id, live] of sessions) {
    const a = app(id);
    if (!a) {
      sessions.delete(id);
      continue;
    }
    if (live.phase.kind === 'open' && now >= live.exitAt) {
      // The main program closed; its helpers stay behind.
      live.running = live.running.filter((t) => t.path !== a.program);
      const wait = (a.waitSeconds ?? settings.waitSeconds) * 1000;
      live.phase = { kind: 'closing', since: now, at: now + (FAST ? Math.min(wait, 1500) : wait) };
    } else if (live.phase.kind === 'closing' && now >= live.phase.at) {
      live.phase = { kind: 'cleaning' };
      setTimeout(() => cleanUp(a), ms(900));
    }
  }
}
if (typeof setInterval === 'function') setInterval(tick, 200);

function startSession(a: App) {
  const now = Date.now();
  const live: Live = { phase: { kind: 'starting', since: now }, openedAt: null, started: [], problems: [], running: [], exitAt: Infinity };
  sessions.set(a.id, live);
  setTimeout(() => {
    for (const r of a.services.filter((r) => r.start)) {
      const s = services.find((x) => x.name === r.name);
      if (s && s.state !== 'running') {
        s.state = 'running';
        live.started.push(s.display);
      }
    }
    setTimeout(() => {
      const opened = Date.now();
      live.phase = { kind: 'open', since: opened };
      live.openedAt = opened;
      live.running = [target(a.program, true), ...(a.id === 'multisim' ? [target(`${NI}\\Shared\\niSvcLoc\\nisvcloc.exe`), target(`${NI}\\Circuit Design Suite 14.3\\NIUpdateService.exe`)] : [target(`${LICENSING}\\ansysli_server.exe`)])];
      live.exitAt = opened + (FAST ? 1200 : 12_000);
    }, ms(a.servicesDelaySeconds * 300));
  }, ms(700));
}

if (SCENARIO === 'closing') {
  const a = apps[0]!;
  const now = Date.now();
  for (const name of NI_SERVICES) services.find((s) => s.name === name)!.state = 'running';
  sessions.set(a.id, {
    phase: { kind: 'closing', since: now - 4000, at: now + 26_000 },
    openedAt: now - 3_600_000,
    started: NI_SERVICES.slice(0, 5).map(display),
    problems: [],
    running: [target(`${NI}\\Shared\\niSvcLoc\\nisvcloc.exe`), target(`${NI}\\Circuit Design Suite 14.3\\NIUpdateService.exe`)],
    exitAt: Infinity,
  });
}

// A week of history for the History page.
if (SCENARIO !== 'empty') {
  const day = 86_400_000;
  const past = (id: string, ago: number, hours: number, fields: Partial<HistoryEntry>) => {
    const a = app(id)!;
    history.push({
      id: `h${history.length}`,
      appId: a.id,
      appName: a.name,
      trigger: 'shortcut',
      outcome: 'cleaned',
      openedAt: Date.now() - ago - hours * 3_600_000,
      endedAt: Date.now() - ago,
      started: [],
      stopped: [],
      closed: [],
      ended: [],
      problems: [],
      ...fields,
    });
  };
  past('multisim', day * 0.2, 2.5, { started: NI_SERVICES.slice(0, 5).map(display), stopped: NI_SERVICES.map(display), closed: ['NIUpdateService.exe'], ended: ['nisvcloc.exe'] });
  past('ansys', day * 1.1, 4, {
    stopped: ANSYS_SERVICES.map(display),
    ended: ['ansysli_server.exe'],
    problems: [{ what: 'ANSYS, Inc. License Manager CVD', code: 'stop_failed', detail: 'it did not finish within 30 s' }],
  });
  past('multisim', day * 2.3, 1, { trigger: 'adopted', outcome: 'kept', stopped: [] });
}

const delay = <T>(value: T, wait = 250): Promise<T> => new Promise((resolve) => setTimeout(() => resolve(structuredClone(value)), ms(wait)));

function status(a: App): AppStatus {
  const live = sessions.get(a.id);
  return {
    id: a.id,
    phase: live?.phase ?? null,
    openedAt: live?.openedAt ?? null,
    running: live?.running ?? leftovers.get(a.id) ?? [],
    services: a.services.map((r) => {
      const s = services.find((x) => x.name === r.name);
      return { name: r.name, state: s?.state ?? null, controllable: s?.controllable ?? false };
    }),
  };
}

const PROGRAMS: Record<string, [string, string | null, boolean][]> = {
  [NI]: [
    ['Circuit Design Suite 14.3\\multisim.exe', 'Multisim', false],
    ['Circuit Design Suite 14.3\\ultiboard.exe', 'Ultiboard', false],
    ['MAX\\NIMax.exe', 'Measurement & Automation Explorer', false],
    ['Shared\\niSvcLoc\\nisvcloc.exe', 'NI Service Locator', false],
    ['Circuit Design Suite 14.3\\NIUpdateService.exe', 'NI Update Service', true],
    ['Shared\\NIUninstaller\\uninst.exe', 'NI Uninstaller', true],
  ],
  [ANSYS_DIR]: [
    ['v241\\Win64\\ansysedt.exe', 'Ansys Electronics Desktop', false],
    ['v241\\Win64\\ansysedtsv.exe', 'Ansys Electronics Desktop Server', false],
    ['v241\\Win64\\Uninstall.exe', 'Uninstall', true],
  ],
};

function pick(): string | null {
  const w = window as unknown as { __stfdPick?: string | null };
  const v = w.__stfdPick;
  return v === undefined ? NI : v;
}

export const mockApi: Api = {
  simulated: true,
  getState: () => delay({ supported: true, now: Date.now(), apps, status: apps.map(status) }, 40),
  getIcon: () => delay(null, 20),
  getSettings: () => delay(settings, 60),
  async setSettings(s) {
    settings = { ...s, waitSeconds: Math.min(3600, Math.max(0, s.waitSeconds)), politeSeconds: Math.min(120, Math.max(1, s.politeSeconds)) };
    return delay(settings, 80);
  },
  getHistory: () => delay(history, 60),
  async clearHistory() {
    history = [];
    await delay(null, 60);
  },
  async inspectFolder(folder) {
    const f = folder.replace(/[\\/]+$/, '');
    const lower = f.toLowerCase();
    let problem: FolderInfo['problem'] = null;
    if (/^[a-z]:$/.test(lower)) problem = { kind: 'driveRoot' };
    else if (lower.startsWith('c:\\windows')) problem = { kind: 'system' };
    else if (['c:\\program files', 'c:\\program files (x86)', 'c:\\programdata', 'c:\\users'].includes(lower)) problem = { kind: 'tooBroad' };
    const list = PROGRAMS[f] ?? [[`${file(f).replace(/\s+/g, '')}.exe`, file(f), false] as [string, string, boolean]];
    const programs: ExeFile[] = problem
      ? []
      : list.map(([rel, description, noise], i) => ({ path: `${f}\\${rel}`, size: 40_000_000 - i * 3_000_000, depth: rel.split('\\').length - 1, gui: !noise, description, noise }));
    return delay({ folder: f, problem, programs, vendorFolders: f === NI ? [NI64] : f === ANSYS_DIR ? ['C:\\Program Files\\ANSYS Inc'] : [] }, 500);
  },
  async inspectProgram(path) {
    const known = Object.entries(PROGRAMS).flatMap(([dir, list]) => list.map(([rel, d]) => [`${dir}\\${rel}`, d] as const)).find(([p]) => p === path);
    const company = path.startsWith(NI) ? 'National Instruments' : path.startsWith(ANSYS_DIR) ? 'ANSYS, Inc.' : null;
    const name = path === MULTISIM ? 'Multisim 14.3' : path === ANSYS ? 'Ansys Electronics Desktop 2024 R1' : (known?.[1] ?? file(path).replace(/\.exe$/i, ''));
    return delay({ path, name, company, description: known?.[1] ?? null }, 120);
  },
  async listServices(folders, program) {
    const company = program.startsWith(NI) ? 'National Instruments' : program.startsWith(ANSYS_DIR) ? 'ANSYS, Inc.' : null;
    const inside = (p: string | null) => !!p && folders.some((f) => p.toLowerCase().startsWith(f.toLowerCase() + '\\'));
    const related: Suggested[] = [];
    for (const s of services) {
      if (inside(s.program)) related.push({ ...s, why: 'inFolder' });
      else if (company && s.company?.toLowerCase().startsWith(company.toLowerCase().split(',')[0]!)) related.push({ ...s, why: 'samePublisher' });
    }
    related.sort((a, b) => Number(a.why === 'samePublisher') - Number(b.why === 'samePublisher') || a.display.localeCompare(b.display));
    const all = [...services].sort((a, b) => a.display.localeCompare(b.display));
    return delay({ company, related, all }, 700);
  },
  async findShortcuts(program) {
    if (program === MULTISIM) return delay([{ path: 'C:\\Users\\Bugra\\Desktop\\Multisim 14.3.lnk', movable: true }], 150);
    if (program === ANSYS) return delay([{ path: 'C:\\Users\\Public\\Desktop\\Ansys Electronics Desktop 2024 R1.lnk', movable: false }], 150);
    return delay([], 150);
  },
  async saveApp(draft: Draft) {
    if (!draft.name.trim()) throw new AppError('invalid', 'name');
    const existing = draft.id ? app(draft.id) : undefined;
    const id = existing?.id ?? Math.random().toString(16).slice(2, 14);
    const saved: App = {
      ...(existing ?? baseApp(id, draft.name, draft.program, draft.folders, [])),
      name: draft.name.trim(),
      program: draft.program,
      arguments: draft.arguments,
      folders: draft.folders,
      services: draft.services.map((s) => ({ ...s, original: { startType: draft.manual.includes(s.name) ? 'automatic' : null, granted: true } })),
      alsoClose: draft.alsoClose,
      keep: draft.keep,
      waitSeconds: draft.waitSeconds,
      servicesDelaySeconds: draft.servicesDelaySeconds,
      closePrograms: draft.closePrograms,
      shortcuts: { made: draft.desktop ? [`C:\\Users\\Bugra\\Desktop\\${draft.name.trim()}.lnk`] : [], moved: draft.replace.map((from) => ({ from, to: `C:\\Users\\Bugra\\AppData\\Local\\com.bugragungoz.stfd\\shortcuts\\${id}\\${file(from)}` })) },
    };
    for (const s of draft.services) {
      const real = services.find((x) => x.name === s.name);
      if (real) {
        real.controllable = true;
        if (draft.manual.includes(s.name)) real.startType = 'manual';
      }
    }
    apps = existing ? apps.map((a) => (a.id === id ? saved : a)) : [...apps, saved];
    return delay({ app: saved, permission: draft.services.length ? ('granted' as const) : ('notNeeded' as const), problems: [] }, 900);
  },
  async updateApp(updated) {
    if (!app(updated.id)) throw new AppError('not_found', updated.id);
    apps = apps.map((a) => (a.id === updated.id ? updated : a));
    return delay(updated, 120);
  },
  async grantPermission(id) {
    const a = app(id);
    if (!a) throw new AppError('not_found', id);
    for (const r of a.services) {
      const s = services.find((x) => x.name === r.name);
      if (s) s.controllable = true;
    }
    return delay({ app: a, permission: 'granted' as const, problems: [] }, 700);
  },
  async removeApp(id) {
    sessions.delete(id);
    apps = apps.filter((a) => a.id !== id);
    return delay({ permission: 'granted' as const, problems: [] }, 600);
  },
  async openApp(id) {
    const a = app(id);
    if (!a) throw new AppError('not_found', id);
    const live = sessions.get(id);
    if (live?.phase.kind === 'closing') {
      live.phase = { kind: 'open', since: Date.now() };
      live.running = [target(a.program, true), ...live.running];
      live.exitAt = Date.now() + (FAST ? 1200 : 12_000);
    } else if (!live) startSession(a);
    await delay(null, 60);
  },
  async previewCleanup(id) {
    const a = app(id);
    if (!a) throw new AppError('not_found', id);
    return delay(planFor(a), 300);
  },
  async closeNow(id) {
    const a = app(id);
    if (!a) throw new AppError('not_found', id);
    const live = sessions.get(id);
    if (live?.phase.kind === 'closing') {
      live.phase = { ...live.phase, at: Date.now() };
      return delay(null, 60);
    }
    return delay(cleanUp(a), 900);
  },
  async keepRunning(id) {
    const a = app(id);
    const live = sessions.get(id);
    if (a && live) {
      sessions.delete(id);
      entry(a, live, 'kept', {});
    }
    await delay(null, 60);
  },
  async makeShortcuts(id, desktop, startMenu) {
    const a = app(id);
    if (!a) throw new AppError('not_found', id);
    const made = [
      ...(desktop ? [`C:\\Users\\Bugra\\Desktop\\${a.name}.lnk`] : []),
      ...(startMenu ? [`C:\\Users\\Bugra\\AppData\\Roaming\\Microsoft\\Windows\\Start Menu\\Programs\\${a.name}.lnk`] : []),
    ];
    const updated = { ...a, shortcuts: { ...a.shortcuts, made } };
    apps = apps.map((x) => (x.id === id ? updated : x));
    return delay(updated, 300);
  },
  pickFolder: () => delay(pick(), 100),
  async pickProgram() {
    const w = window as unknown as { __stfdPickProgram?: string | null };
    return delay(w.__stfdPickProgram ?? null, 100);
  },
  async openUrl(url) {
    window.open(url, '_blank', 'noopener');
  },
  async reveal() {},
};
