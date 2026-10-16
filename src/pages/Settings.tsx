import { api } from '../api';
import type { Settings as SettingsT } from '../api/types';
import { Button } from '../components/Button';
import { Icon } from '../components/Icon';
import { PageHeader, Segmented } from '../components/Misc';
import { Switch } from '../components/Switch';
import { LANGUAGES, t } from '../i18n';
import { errorMessage } from '../lib/errors';
import { formatWait } from '../lib/format';
import { saveSettings, settings } from '../state/settings';
import { toast } from '../state/toasts';

export const REPO_URL = 'https://github.com/bugragungoz/smart-launcher-scripts';

const WAITS = [0, 10, 30, 60, 120, 300];
const POLITE = [5, 10, 20, 30];

function Group({ title, children }: { title: string; children: preact.ComponentChildren }) {
  return (
    <section class="settings-group">
      <h2 class="settings-title">{title}</h2>
      <div class="settings-body">{children}</div>
    </section>
  );
}

export function Settings() {
  const s = settings.value;
  if (!s) return null;
  const set = (patch: Partial<SettingsT>) => void saveSettings(patch).catch((e) => toast(errorMessage(e), { tone: 'warn' }));
  return (
    <section class="view">
      <div class="view-body">
        <PageHeader title={t('settings.title')} />
        <Group title={t('settings.closing.title')}>
          <div class="setting-row">
            <span class="setting-label">{t('settings.wait')}</span>
            <Segmented<`${number}`> label={t('settings.wait')} value={`${s.waitSeconds}`} options={WAITS.map((w) => ({ value: `${w}` as `${number}`, label: formatWait(w) }))} onChange={(v) => set({ waitSeconds: Number(v) })} />
            <span class="setting-meta">{t('settings.waitHint')}</span>
          </div>
          <div class="setting-row">
            <span class="setting-label">{t('settings.polite')}</span>
            <Segmented<`${number}`> label={t('settings.polite')} value={`${s.politeSeconds}`} options={POLITE.map((w) => ({ value: `${w}` as `${number}`, label: formatWait(w) }))} onChange={(v) => set({ politeSeconds: Number(v) })} />
            <span class="setting-meta">{t('settings.politeHint')}</span>
          </div>
          <Switch checked={s.notify} onChange={(v) => set({ notify: v })} label={t('settings.notify')} hint={t('settings.notifyHint')} />
        </Group>
        <Group title={t('settings.watch.title')}>
          <Switch checked={s.watchAlways} onChange={(v) => set({ watchAlways: v })} label={t('settings.watchAlways')} hint={t('settings.watchHint')} />
        </Group>
        <Group title={t('settings.look.title')}>
          <div class="setting-row">
            <span class="setting-label">{t('settings.language')}</span>
            <Segmented label={t('settings.language')} value={s.language} options={[{ value: 'system', label: t('settings.system') }, ...LANGUAGES.map((l) => ({ value: l.code, label: l.nativeName }))]} onChange={(v) => set({ language: v })} />
          </div>
          <div class="setting-row">
            <span class="setting-label">{t('settings.theme')}</span>
            <Segmented<SettingsT['theme']>
              label={t('settings.theme')}
              value={s.theme}
              options={[
                { value: 'system', label: t('settings.system') },
                { value: 'dark', label: t('settings.dark') },
                { value: 'light', label: t('settings.light') },
              ]}
              onChange={(v) => set({ theme: v })}
            />
          </div>
        </Group>
        <Group title={t('settings.about.title')}>
          <p class="about-text">{t('settings.about.body')}</p>
          <p class="setting-meta">{t('status.version', { version: __APP_VERSION__ })}</p>
          <div>
            <Button size="sm" onClick={() => void api.openUrl(REPO_URL)}>
              <Icon name="github" size={16} />
              {t('settings.about.github')}
            </Button>
          </div>
        </Group>
      </div>
    </section>
  );
}
