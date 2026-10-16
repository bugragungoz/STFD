import { iconFor } from '../state/apps';

/** A program's own icon, or its first letter in a tile until (or unless) there is one. */
export function AppIcon({ path, name, size = 40 }: { path: string; name: string; size?: number }) {
  const url = iconFor(path);
  if (url) return <img class="app-icon" src={url} width={size} height={size} alt="" />;
  const letter = (name.trim()[0] ?? '?').toLocaleUpperCase();
  return (
    <span class="app-icon app-icon-letter" style={{ width: `${size}px`, height: `${size}px`, fontSize: `${Math.round(size * 0.45)}px` }} aria-hidden="true">
      {letter}
    </span>
  );
}
