import { describe, expect, it } from 'vitest';
import type { App } from '../api/types';
import { closes } from './AppDetail';

const app = {
  program: 'C:\\NI\\Suite\\multisim.exe',
  folders: ['C:\\NI'],
  keep: ['C:\\NI\\MAX\\NIMax.exe'],
  alsoClose: ['C:\\Users\\b\\AppData\\Local\\NI\\crash.exe'],
} as App;

describe('closes', () => {
  it('matches what the core closes after an app', () => {
    expect(closes(app, 'c:\\ni\\shared\\nisvcloc.exe')).toBe(true);
    expect(closes(app, 'C:\\NI\\MAX\\NIMax.exe')).toBe(false);
    expect(closes(app, 'C:\\Users\\b\\AppData\\Local\\NI\\crash.exe')).toBe(true);
    expect(closes(app, 'C:\\NI Tools\\x.exe')).toBe(false);
    expect(closes(app, 'C:\\Other\\x.exe')).toBe(false);
  });
});
