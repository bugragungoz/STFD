import { describe, expect, it } from 'vitest';
import type { App } from '../api/types';
import { closes } from './AppDetail';

const app = {
  program: 'C:\\Lab\\Suite\\circuitsim.exe',
  folders: ['C:\\Lab'],
  keep: ['C:\\Lab\\Explorer\\hwexplorer.exe'],
  alsoClose: ['C:\\Users\\b\\AppData\\Local\\Lab\\crash.exe'],
} as App;

describe('closes', () => {
  it('matches what the core closes after an app', () => {
    expect(closes(app, 'c:\\lab\\shared\\elsvcloc.exe')).toBe(true);
    expect(closes(app, 'C:\\Lab\\Explorer\\hwexplorer.exe')).toBe(false);
    expect(closes(app, 'C:\\Users\\b\\AppData\\Local\\Lab\\crash.exe')).toBe(true);
    expect(closes(app, 'C:\\Lab Tools\\x.exe')).toBe(false);
    expect(closes(app, 'C:\\Other\\x.exe')).toBe(false);
  });
});
