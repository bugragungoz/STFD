/** Where the window is: the page and the app it shows. */
import { signal } from '@preact/signals';

export type Page = 'apps' | 'app' | 'add' | 'history' | 'settings';

export const page = signal<Page>('apps');
export const openId = signal<string | null>(null);

export function showApp(id: string): void {
  openId.value = id;
  page.value = 'app';
}

export function showPage(p: Page): void {
  page.value = p;
}
