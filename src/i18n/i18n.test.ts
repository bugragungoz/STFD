import { afterEach, describe, expect, it } from 'vitest';
import { formatNumber, setLanguage, t } from '.';

afterEach(() => setLanguage('en'));

describe('t', () => {
  it('fills placeholders and picks the plural form from count', () => {
    setLanguage('en');
    expect(t('apps.services', { count: 1 })).toBe('1 service');
    expect(t('apps.services', { count: 7 })).toBe('7 services');
    expect(t('phase.closing', { time: '0:25' })).toBe('Closing the rest in 0:25');
  });

  it('writes numbers the way the language does', () => {
    setLanguage('tr');
    expect(formatNumber(1234.5)).toBe('1.234,5');
    expect(t('history.stopped', { count: 1200 })).toBe('1.200 hizmet durduruldu');
  });

  it('switches language and falls back to the key for an unknown message', () => {
    setLanguage('tr');
    expect(t('nav.apps')).toBe('Uygulamalar');
    expect(t('no.such.key')).toBe('no.such.key');
    setLanguage('xx');
    expect(t('nav.apps')).toBe('Apps');
  });
});
