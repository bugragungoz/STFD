// The built interface in Chromium against the simulated Windows (src/api/mock.ts): every page
// opens without an error, an app is added in four steps, a folder full of unrelated programs is
// refused, an opened app is followed until what it left is closed, the wait can be cut short or
// skipped, leftovers can be closed by hand after a look at what goes, an app is removed only after a
// question, the language follows the setting, and the smallest window has no sideways scroll.
//   npm run test:ui   (builds first; CHROMIUM_PATH picks a browser when Playwright finds none)
import assert from 'node:assert/strict';
import { after, before, test } from 'node:test';
import { launch } from './browser.mjs';
import { serve } from './server.mjs';

let server;
let browser;

before(async () => {
  server = await serve();
  browser = await launch();
});
after(async () => {
  await browser?.close();
  server?.close();
});

async function open({ scenario, width = 1280, height = 800, locale = 'en-US' } = {}) {
  const context = await browser.newContext({ viewport: { width, height }, locale, colorScheme: 'dark' });
  const page = await context.newPage();
  const problems = [];
  page.on('pageerror', (e) => problems.push(e.message));
  page.on('console', (m) => {
    if (m.type() === 'error' || m.type() === 'warning') problems.push(m.text());
  });
  page.on('response', (r) => {
    if (r.status() >= 400) problems.push(`${r.status()} ${r.url()}`);
  });
  await page.goto(`${server.url}?fast${scenario ? `&scenario=${scenario}` : ''}`);
  await page.getByRole('heading', { name: /^(Apps|Uygulamalar)$/ }).waitFor();
  return { page, context, problems };
}

const nav = (page, name) => page.getByRole('navigation').getByRole('button', { name: new RegExp(`^${name}`) }).click();
const card = (page, name) => page.getByRole('list', { name: 'Apps' }).getByRole('button', { name: new RegExp(`^${name}`) });

test('every page opens without an error', async () => {
  const { page, context, problems } = await open();
  await card(page, 'Circuit Simulator 14').click();
  await page.getByRole('heading', { name: 'Circuit Simulator 14', exact: true }).waitFor();
  await page.getByRole('heading', { name: 'Services' }).waitFor();
  for (const [name, heading] of [
    ['History', 'History'],
    ['Settings', 'Settings'],
    ['Apps', 'Apps'],
  ]) {
    await nav(page, name);
    await page.getByRole('heading', { name: heading, exact: true }).waitFor();
  }
  await page.getByRole('button', { name: 'Add app' }).click();
  await page.getByRole('heading', { name: 'Add an app' }).waitFor();
  assert.deepEqual(problems, []);
  await context.close();
});

test('an app is added in four steps, with its services found in its folder', async () => {
  const { page, context, problems } = await open({ scenario: 'empty' });
  await page.getByRole('heading', { name: 'How STFD works' }).waitFor();
  await page.getByRole('button', { name: 'Add your first app' }).click();

  // Nothing to go on before a folder is chosen.
  const next = page.getByRole('button', { name: /^Next/ });
  assert.equal(await next.isDisabled(), true);
  await page.getByRole('button', { name: 'Choose folder' }).click();
  await page.getByText('Found 6 programs.').waitFor();

  // The likely program is chosen, and its name comes from the program itself.
  await next.click();
  await page.getByRole('radio', { name: /^Circuit Simulator/ }).waitFor();
  assert.equal(await page.getByRole('radio', { name: /^Circuit Simulator/ }).getAttribute('aria-checked'), 'true');
  assert.equal(await page.getByLabel('Name').inputValue(), 'Circuit Simulator 14');
  assert.equal(await page.getByRole('radio', { name: /Uninstaller/ }).count(), 0, 'uninstallers are hidden');

  await next.click();
  await page.getByText('7 services, publisher Example Labs').waitFor();
  assert.equal(await page.getByRole('checkbox', { name: 'Use Example Labs Domain Service' }).isChecked(), true);
  await page.getByRole('checkbox', { name: 'Use Example Labs Time Synchronization' }).uncheck();
  // Two of them start with Windows; by default they start only when needed from now on.
  assert.equal(await page.getByRole('switch', { name: 'Start 2 of them only when needed' }).getAttribute('aria-checked'), 'true');

  await next.click();
  await page.getByText('6 services · 1 folder').waitFor();
  assert.equal(await page.getByRole('checkbox', { name: 'Put STFD\'s shortcut in place of "Circuit Simulator 14" on the desktop' }).isChecked(), true);
  await page.getByText('Windows will ask once').waitFor();
  await page.getByRole('button', { name: 'Add app' }).click();

  await page.getByRole('heading', { name: 'Circuit Simulator 14', exact: true }).waitFor();
  await page.getByText('Circuit Simulator 14 is ready. Open it with the new shortcut on your desktop.').waitFor();
  await page.getByText('Example Labs Domain Service').first().waitFor();
  assert.equal(await page.getByText('Example Labs Time Synchronization').count(), 0);
  await nav(page, 'Apps');
  await card(page, 'Circuit Simulator 14').waitFor();
  assert.deepEqual(problems, []);
  await context.close();
});

test('a folder that holds unrelated programs is refused', async () => {
  const { page, context } = await open();
  await page.evaluate(() => (window.__stfdPick = 'C:\\Program Files'));
  await page.getByRole('button', { name: 'Add app' }).click();
  await page.getByRole('button', { name: 'Choose folder' }).click();
  await page.getByText('This folder holds many unrelated programs').waitFor();
  assert.equal(await page.getByRole('button', { name: /^Next/ }).isDisabled(), true);
  await context.close();
});

test('an opened app is followed until what it left is closed', async () => {
  const { page, context, problems } = await open();
  await card(page, 'Circuit Simulator 14').click();
  await page.getByRole('button', { name: 'Open', exact: true }).click();
  await page.getByText('Opening Circuit Simulator 14').waitFor();
  const now = page.getByRole('region', { name: 'Now' });
  await now.getByText('Open', { exact: true }).waitFor();
  await now.getByText('circuitsim.exe', { exact: true }).waitFor();
  // The program closes on its own; its helpers stay until the wait runs out.
  await now.getByText(/^Closing the rest in/).waitFor();
  await now.getByText('elsvcloc.exe', { exact: true }).waitFor();
  await now.getByText('Not open', { exact: true }).waitFor({ timeout: 10_000 });
  await nav(page, 'History');
  const first = page.getByRole('listitem').first();
  await first.getByText('Cleaned up').waitFor();
  await first.getByText('stopped 7 services').waitFor();
  assert.deepEqual(problems, []);
  await context.close();
});

test('the wait can be cut short or skipped', async () => {
  const { page, context } = await open({ scenario: 'closing' });
  await card(page, 'Circuit Simulator 14').click();
  const now = page.getByRole('region', { name: 'Now' });
  await now.getByText(/^Closing the rest in 0:2/).waitFor();
  await now.getByRole('button', { name: 'Leave running' }).click();
  await page.getByText('Circuit Simulator 14: everything stays running this time.').waitFor();
  await now.getByText('Not open', { exact: true }).waitFor();
  await nav(page, 'History');
  await page.getByRole('listitem').first().getByText('Left running').waitFor();
  await context.close();

  const second = await open({ scenario: 'closing' });
  await card(second.page, 'Circuit Simulator 14').click();
  const region = second.page.getByRole('region', { name: 'Now' });
  await region.getByRole('button', { name: 'Close now' }).click();
  await region.getByText('Not open', { exact: true }).waitFor({ timeout: 5000 });
  await second.context.close();
});

test('leftovers are closed by hand after a look at what goes', async () => {
  const { page, context } = await open();
  await card(page, 'Field Solver 2024 R1').click();
  await page.getByRole('region', { name: 'Now' }).getByText('licserver.exe', { exact: true }).waitFor();
  await page.getByRole('button', { name: 'Close leftovers now' }).click();
  const dialog = page.getByRole('dialog', { name: 'Close what Field Solver 2024 R1 left?' });
  await dialog.getByText('1 program').waitFor();
  await dialog.getByText('2 services to stop').waitFor();
  await dialog.getByText('Sample Systems Licensing Web Service').waitFor();
  await dialog.getByRole('button', { name: 'Close them' }).click();
  await page.getByText('Stopped 2 services and closed 1 program.').waitFor();
  await page.getByRole('region', { name: 'Now' }).getByText('Not open. Its shortcut opens it with STFD.').waitFor();
  await context.close();
});

test('an app is removed only after a question', async () => {
  const { page, context } = await open();
  await card(page, 'Circuit Simulator 14').click();
  await page.getByRole('button', { name: 'Remove', exact: true }).click();
  const dialog = page.getByRole('dialog', { name: 'Remove Circuit Simulator 14?' });
  await dialog.getByText('Windows asks once for permission').waitFor();
  await dialog.getByRole('button', { name: 'Cancel' }).click();
  await page.getByRole('heading', { name: 'Circuit Simulator 14', exact: true }).waitFor();
  await page.getByRole('button', { name: 'Remove', exact: true }).click();
  await page.getByRole('dialog').getByRole('button', { name: 'Remove' }).click();
  await page.getByText('Circuit Simulator 14 was removed, with its shortcuts.').waitFor();
  assert.equal(await card(page, 'Circuit Simulator 14').count(), 0);
  await context.close();
});

test('a service rule changes on the app page', async () => {
  const { page, context } = await open();
  await card(page, 'Circuit Simulator 14').click();
  const row = page.getByRole('listitem').filter({ hasText: 'Example Labs Time Synchronization' });
  const start = row.getByRole('checkbox', { name: 'Start before' });
  assert.equal(await start.isChecked(), true);
  await start.uncheck();
  await nav(page, 'Apps');
  await card(page, 'Circuit Simulator 14').click();
  assert.equal(await page.getByRole('listitem').filter({ hasText: 'Example Labs Time Synchronization' }).getByRole('checkbox', { name: 'Start before' }).isChecked(), false);
  await context.close();
});

test('the language follows the setting', async () => {
  const { page, context } = await open();
  await nav(page, 'Settings');
  await page.getByRole('radio', { name: 'Türkçe' }).click();
  await page.getByRole('heading', { name: 'Ayarlar', exact: true }).waitFor();
  await nav(page, 'Uygulamalar');
  await page.getByText('7 hizmet · 30 sn bekler').waitFor();
  await context.close();
});

test('the smallest window has no sideways scroll', async () => {
  const { page, context } = await open({ width: 900, height: 600, scenario: 'closing' });
  const wide = async (where) => {
    const over = await page.evaluate(() => [...document.querySelectorAll('.view-body')].some((el) => el.scrollWidth > el.clientWidth + 1) || document.documentElement.scrollWidth > window.innerWidth);
    assert.equal(over, false, where);
  };
  await wide('apps');
  await card(page, 'Circuit Simulator 14').click();
  await page.getByRole('heading', { name: 'Services' }).waitFor();
  await wide('app');
  await nav(page, 'History');
  await page.getByRole('button', { name: 'Details' }).first().click();
  await wide('history');
  await nav(page, 'Settings');
  await wide('settings');
  await nav(page, 'Apps');
  await page.getByRole('button', { name: 'Add app' }).click();
  await page.getByRole('button', { name: 'Choose folder' }).click();
  await page.getByText('Found 6 programs.').waitFor();
  await page.getByRole('button', { name: /^Next/ }).click();
  await wide('add');
  await context.close();
});
