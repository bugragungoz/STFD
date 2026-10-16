// The README screenshots, from the simulated Windows (made-up Multisim and Ansys installs).
//   npm run shots   ->  docs/images/*.png
import { mkdir } from 'node:fs/promises';
import { fileURLToPath } from 'node:url';
import { launch } from './browser.mjs';
import { serve } from './server.mjs';

const OUT = fileURLToPath(new URL('../../docs/images/', import.meta.url));
await mkdir(OUT, { recursive: true });
const server = await serve();
const browser = await launch();

async function shoot(name, { colorScheme = 'dark', scenario = '', steps = async () => {} } = {}) {
  const context = await browser.newContext({ viewport: { width: 1280, height: 800 }, deviceScaleFactor: 1, colorScheme, locale: 'en-US' });
  const page = await context.newPage();
  await page.goto(`${server.url}?fast${scenario ? `&scenario=${scenario}` : ''}`);
  await page.getByRole('heading', { name: 'Apps', exact: true }).waitFor();
  await steps(page);
  await page.waitForTimeout(500);
  await page.screenshot({ path: `${OUT}${name}.png` });
  await context.close();
  console.log(`docs/images/${name}.png`);
}

const card = (page, name) => page.getByRole('list', { name: 'Apps' }).getByRole('button', { name: new RegExp(`^${name}`) });
const toServices = async (page) => {
  await page.getByRole('button', { name: 'Add app' }).click();
  await page.getByRole('button', { name: 'Choose folder' }).click();
  await page.getByText('Found 6 programs.').waitFor();
  await page.getByRole('button', { name: /^Next/ }).click();
  await page.getByRole('button', { name: /^Next/ }).click();
  await page.getByText('7 services, publisher National Instruments').waitFor();
};

await shoot('apps', { scenario: 'closing' });
await shoot('app', {
  scenario: 'closing',
  steps: async (page) => {
    await card(page, 'Multisim 14.3').click();
    await page.getByRole('heading', { name: 'Services' }).waitFor();
  },
});
await shoot('add', { colorScheme: 'light', steps: toServices });
await shoot('history', {
  colorScheme: 'light',
  steps: async (page) => {
    await page.getByRole('navigation').getByRole('button', { name: /^History/ }).click();
    await page.getByRole('button', { name: 'Details' }).nth(1).click();
  },
});

await browser.close();
server.close();
