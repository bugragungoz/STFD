# What STFD adds to Bugra

Bugra was drawn for a website. STFD reuses its tokens, fonts and components (copied from the
owner's component-inventory app through netSwitch and adbDebloat) and adds the pieces a launcher
needs. Each one is built from Bugra tokens and shapes and is marked "(added)" in the code. No color
is added: every color comes from `src/styles/tokens.css`, which `npm run check:colors` enforces.

## Components

| Piece | Where | Notes |
|---|---|---|
| Button `quiet` and `danger` | `components/Button.tsx` | Quiet has no outline until hovered; danger uses the `warn` outline and text. |
| Checkbox | `components/Switch.tsx` | Native input, 3:1 outline, accent fill with a check. |
| Badge | `components.css` | A small outlined pill. `badge-ok`, `badge-info` and `badge-warn` use the status tokens and always carry a word, often an icon, never color alone. |
| Phase badge | `components/PhaseBadge.tsx` | Not open (neutral), Starting (info), Open (ok, play), Closing the rest in 0:25 (info, timer), Closing the rest (info). |
| App icon | `components/AppIcon.tsx`, `layout.css` | The program's own icon; until it arrives (or when it has none), its first letter on `surface-2` in the accent ink. |
| App card | `pages/Apps.tsx`, `layout.css` | A bordered row: icon, name, a quiet summary, the phase badge and Open. |
| Steps | `pages/AddApp.tsx`, `.stepper` | Bugra's step badge (`--step-badge`): accent fill for the current step, accent outline with a check for done ones. |
| Pick list | `layout.css`, `.pick-list` | Rows in one bordered box; programs are radios (accent dot), services checkboxes. |
| Segmented control | `components/Misc.tsx`, `.tabs` | A pill group with `role="radiogroup"`; the chosen option wears the accent tint. |
| Snackbar stack | `components/Snackbars.tsx` | Bugra's snackbar, stacked at the bottom left with a close button. |
| Status bar | `App.tsx`, `layout.css` | What STFD follows, the version and the GitHub link. |

## Icons (`components/icons.ts`)

Bugra-style line icons on a 24 px grid, 1.6 px stroke with round caps. STFD adds `plus`,
`play`, `folder`, `link`, `window`, `timer`, `shield`, `stop`, `back` and `services`, the mark
beside the name in the sidebar (`App.tsx`: a program with the two things that come and go with it),
and the GitHub mark from Primer Octicons (MIT) for the link in the status bar.

The app icon (`docs/design/icon.svg`, rendered into `src-tauri/icons` with `npm run icons`) is the
sidebar mark in the accent on the dark background.
