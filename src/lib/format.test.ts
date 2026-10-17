import { afterEach, describe, expect, it } from 'vitest';
import { setLanguage } from '../i18n';
import { fileName, formatCountdown, formatSpan, formatWait, relativeTo } from './format';

afterEach(() => setLanguage('en'));

describe('format', () => {
  it('says how long a wait is', () => {
    setLanguage('en');
    expect(formatWait(0)).toBe('No wait');
    expect(formatWait(30)).toBe('30 s');
    expect(formatWait(120)).toBe('2 min');
    expect(formatWait(90)).toBe('1 min 30 s');
    setLanguage('tr');
    expect(formatWait(300)).toBe('5 dk');
  });

  it('counts down in minutes and seconds, never below zero', () => {
    expect(formatCountdown(25_000)).toBe('0:25');
    expect(formatCountdown(125_400)).toBe('2:06');
    expect(formatCountdown(-5)).toBe('0:00');
  });

  it('says how long something lasted', () => {
    setLanguage('en');
    expect(formatSpan(40_000)).toBe('40 s');
    expect(formatSpan(12 * 60_000)).toBe('12 min');
    expect(formatSpan(125 * 60_000)).toBe('2 h 5 min');
  });

  it('shows paths relative to the folder they are in', () => {
    expect(relativeTo('C:\\Lab\\Suite\\circuitsim.exe', 'c:\\lab\\')).toBe('Suite\\circuitsim.exe');
    expect(relativeTo('D:\\Other\\x.exe', 'C:\\Lab')).toBe('D:\\Other\\x.exe');
    expect(fileName('C:\\Lab\\circuitsim.exe')).toBe('circuitsim.exe');
  });
});
