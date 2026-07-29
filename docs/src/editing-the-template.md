# Editing the template

`deck init` gives you a working three-slide deck rather than an empty directory. This is
a tour of what to change, roughly in the order you will want to change it.

```text
my-deck/
├── deck.toml
├── slides/00-title.html         ← 1. the title
├── slides/10-overview.html      ← 2. an example with steps
├── slides/20-architecture.html  ← 3. an example with cards and slide-local JS
├── design/tokens.css            ← 4. sizes and fonts
├── design/theme.css             ← 5. colours and component tweaks
├── design/tailwind.css               extra @theme / @utility / @apply
├── design/overrides.css              last word, for one-off fixes
├── components/index.js          ← 6. where your components are registered
├── components/example-badge.js       delete once you have your own
└── assets/README.md                  the conventions, in the directory itself
```

## 1. Deck metadata

```toml
# deck.toml
[deck]
title = "Chelamon Architecture"
author = "Junta Goto"
lang = "en"                       # set "ja" for a Japanese deck
```

`title` is used by the browser tab, the index page and the print view. `lang` lands on
`<html lang>` in the pages deck generates — your slide files carry their own.

## 2. The slides

Rewrite them; do not start from scratch. Each one demonstrates something worth keeping:

- **`00-title.html`** — `layout="title"`, `deck-heading` with `eyebrow` and `sub`, and a
  Tailwind gradient. Replace the text and you have your title slide.
- **`10-overview.html`** — three `data-step` reveals, numbered with Tailwind utilities.
  A good place to see how steps behave before you rely on them.
- **`20-architecture.html`** — `deck-grid` with two `deck-card`s, a `deck-callout`, and a
  `<script type="module">` at the bottom that reacts to `deck:stepchange`. This is the
  pattern for slide-local behaviour.

Then add your own:

```bash
deck add slide security --after architecture
```

## 3. The look

Change a **token** before you change anything else — one value restyles the whole deck:

```css
/* design/theme.css */
:root {
  --deck-color-accent: #4338ca;
}
```

The four files in `design/` differ only in cascade order, and
[Styling and theming](./styling.md) explains which one to reach for. A useful default:
sizes and fonts in `tokens.css`, colours and component tweaks in `theme.css`, Tailwind
additions in `tailwind.css`, and `overrides.css` for the fix you are not proud of.

## 4. Fonts

Drop a webfont into `assets/fonts/` and point a token at it. No `@font-face` to write —
the file name carries the metadata:

```css
/* design/tokens.css */
:root {
  --deck-font-sans: "Inter", sans-serif;
}
```

See [Assets, fonts and the cursor](./assets.md).

## 5. Components

`components/example-badge.js` exists to be read once and deleted. When you want your
own:

```bash
deck component new acme-metric
```

See [Adding your own components](./own-components.md).

## 6. Check as you go

```bash
deck check
```

A freshly generated deck passes with no errors and no warnings, so anything the check
reports is something you introduced. Keep it that way and the deck stays presentable.
