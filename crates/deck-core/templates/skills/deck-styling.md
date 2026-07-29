---
name: deck-styling
description: Change how this deck looks. Use when adjusting colours, fonts, sizes or spacing, editing anything under design/ (tokens.css, theme.css, overrides.css, tailwind.css), adding a webfont or a custom cursor to assets/, switching theme, writing Tailwind utilities or @theme rules, reasoning about which CSS wins, or fixing a low_contrast or min_font_size check.
---

# Styling this deck

Four project stylesheets, one Tailwind entry, and a cascade order that is fixed
so you never have to fight it. Sibling skills: **deck-slides** for slide
markup, **deck-components** for Custom Elements. Full guide:
<https://azishio.github.io/deck/styling.html>.

## Where things belong

| File | For |
|---|---|
| `design/tokens.css` | `--deck-*` values: sizes, fonts, spacing |
| `design/theme.css` | the project look: colours, component tweaks |
| `design/overrides.css` | the last word, for one-off fixes |
| `design/tailwind.css` | `@theme`, `@utility`, `@apply` |

The first three are listed in `[theme].styles` in `deck.toml` and loaded in that
order, so `overrides.css` wins ties. They are all in the same CSS layer; the
split is a convention for humans, not a mechanism.

## Change a token first

Every built-in style is driven by a custom property, so one value restyles the
whole deck. Do this before overriding a component.

```css
/* design/theme.css */
:root {
  --deck-color-accent: #4338ca;
  --deck-font-size-body: 26px;
  --deck-slide-padding-x: 80px;
}
```

Families: `--deck-color-*`, `--deck-font-*`, `--deck-font-size-*`,
`--deck-space-*`, `--deck-radius-*`, `--deck-slide-padding-*`, `--deck-step-*`.
`deck component show deck-card` prints which ones a component actually reads.

## Tailwind CSS

Tailwind is part of the runtime, not an add-on: the vendored v4 **browser build**
compiles utilities inside each slide, with no build step and no network. The deck
tokens are bridged into the Tailwind theme, so utilities follow the theme instead
of duplicating it — `bg-surface`, `text-accent`, `text-small`, `font-mono`,
`rounded-card`.

Project additions go in `design/tailwind.css`:

```css
@theme {
  --color-brand: oklch(0.6 0.18 25);
}

@utility slide-lead {
  font-size: var(--deck-font-size-body);
  color: var(--deck-color-muted);
}
```

Prefer a utility class over a one-off `<style>`, and a token over a utility
repeated on twenty elements.

## The cascade

Declared in both `design.css` and the Tailwind entry, lowest priority first:

```css
@layer base, theme, deck, project, slide, components, utilities;
@layer deck.reset, deck.tokens, deck.base, deck.components;
```

`base` and `theme` are Tailwind's reset and variables, `deck` is the design
system, `project` is `design/`, `slide` is per-slide CSS, and Tailwind's
`utilities` win over everything. So your CSS outranks the design system, and a
utility outranks your CSS.

`deck.base` is the `base` **sub-layer of `deck`**, not Tailwind's top-level
`base`. Keep the two statements separate; merging them sinks the whole design
system below preflight.

Slide-local CSS must be wrapped, or it becomes unlayered and jumps above the
utilities:

```html
<style>
  @layer slide {
    #architecture deck-card { align-items: center; }
  }
</style>
```

## Fonts

Drop files into `assets/fonts/` — no `@font-face` to write. The file name
carries the metadata:

```text
Inter.woff2                    -> Inter, weight 400, normal
Inter-700.woff2                -> Inter, weight 700
Inter-SemiBold.woff2           -> Inter, weight 600
NotoSansJP-Bold-Italic.woff2   -> NotoSansJP, weight 700, italic
```

`.woff2`, `.woff`, `.ttf`, `.otf` are recognised; weights may be numeric or
named; `italic`/`oblique` sets the style. They load with `font-display: block`,
because a slide should never flash its fallback in front of an audience.

Then point a token at the family:

```css
/* design/tokens.css */
:root {
  --deck-font-sans: "Inter", sans-serif;
  --deck-font-mono: "JetBrains Mono", ui-monospace, monospace;
}
```

`deck check` reports `missing_font` if a family is referenced but never loads,
and `deck doctor` lists the families it can find.

## Custom cursor

Put `cursor.svg` (or `.png`, `.webp`, `.gif`, `.jpg`) directly in `assets/` and
it replaces the mouse cursor on every slide and in the presentation view. An SVG
cursor needs explicit `width`/`height`, and browsers ignore images larger than
128×128 — 24–32px with the hotspot at the top-left behaves as people expect.

## Taking a built-in apart

```bash
deck component show deck-card      # read its built-in styles
deck component eject deck-card     # copy them into design/ejected/
```

Add the ejected file to `[theme].styles` and edit freely; the component keeps
working.

## Themes

`deck init --theme` only chooses the starting content of `design/theme.css`
(`default`, `minimal-light`, `dark`). After `init` there is no theme system to
fight — the theme is just your CSS.

## Checking

```bash
deck check
```

Styling changes are what `low_contrast`, `min_font_size`, `clipped_text` and
`slide_overflow` catch. Contrast is measured against the actual computed
background, so a tinted surface with muted text will be reported even though it
looked fine to you.
