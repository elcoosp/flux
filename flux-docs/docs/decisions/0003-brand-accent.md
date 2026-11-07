# 0003 — Brand accent derived from the logo

- **Status:** Accepted
- **Date:** 2026-11-07
- **Deciders:** Documentation Architect (autonomous run)
- **Supersedes:** none
- **Superseded by:** none

## Context

Starlight ships a default blue accent. Flux's real brand is its logo: a dark
"F" mark split by an amber lightning bolt. The dominant brand color in the
logo raster (`docs/logo.png` in the repository, 1024×1024) is the bolt at
**#F2A018** (hsl(35, 89%, 52%)); the mark itself is near-black. The
repository's vector mark (`website/public/favicon.svg`) renders `#000` in
light scheme and `#fff` in dark scheme.

## Decision

1. **Logo.** Use the repository's own vector mark as the navbar logo with
   light/dark variants generated from it (`logo-light.svg` fill `#000`,
   `logo-dark.svg` fill `#fff`). The mark does not contain the project
   name, so the "Flux" title text stays visible next to it. The raster logo
   is not used in the navbar because it has a baked-in background gradient.
2. **Accent.** Derive the accent from the bolt hue (≈35°) and replace
   Starlight's default blue entirely, per theme via `data-theme`:

| Variable | Light | Dark |
| :-- | :-- | :-- |
| `--sl-color-accent-low` | `hsl(36 100% 93%)` | `hsl(33 70% 16%)` |
| `--sl-color-accent` | `hsl(35 89% 34%)` (#A4640A) | `hsl(37 92% 58%)` |
| `--sl-color-accent-high` | `hsl(28 85% 30%)` (#8E480B) | `hsl(41 100% 72%)` (#FFD270) |

3. **Contrast, computed and verified.** Relative-luminance contrast ratios
   for the shipped values:

| Pair | Ratio | Requirement | Pass |
| :-- | --: | :-- | :-- |
| light accent-high (#8E480B) on white | 6.81:1 | ≥ 4.5 (body text) | Yes |
| white on light accent (#A4640A) | 4.77:1 | ≥ 4.5 (normal text/UI) | Yes |
| dark accent-high (#FFD270) on dark bg (≈#0F0F14) | 13.38:1 | ≥ 4.5 (body text) | Yes |
| dark accent on dark bg (≈#0F0F14) | 9.82:1 | ≥ 4.5 | Yes |

The raw bolt (#F2A018) alone is 2.14:1 on white — decorative only, never
used for text; the shipped values darken/adjust it while keeping the hue.

## Consequences

- No Starlight default blue remains anywhere in the chrome.
- The logo is readable in both themes (black on light, white on dark).
- The light/dark toggle keeps working; both themes were verified during the
  build.
- If the project's logo changes, re-derive the palette and re-run the
  contrast computation in one change.
