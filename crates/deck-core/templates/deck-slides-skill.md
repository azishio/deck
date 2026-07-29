---
name: deck-slides
description: Author and edit slides in this deck. Use when adding or rewriting a slide under slides/, changing the deck's look through design/, fixing a `deck check` violation, or answering questions about how this deck is structured. Covers the file conventions, the built-in deck-* components, the step model, Tailwind usage and the CLI.
---

# Authoring slides in this deck

This project is a **deck**: a slide deck where one slide is one complete HTML
document. There is no slide DSL and no build step. Edit HTML, CSS and
JavaScript directly.

## Ground rules

1. **One file, one slide, one complete HTML document** — `<!doctype html>`
   through `</html>`, with its own `<head>`.
2. **The canvas is exactly 1280×720 CSS pixels** (see `[canvas]` in
   `deck.toml`). Never write viewport-relative layout: no `vw`, `vh`, or media
   queries. The presentation shell scales the whole slide.
3. **Content goes in child elements, never in attributes.** A JSON blob in an
   attribute is a slide DSL in disguise.
4. **Reference assets with root-absolute paths** — `/assets/images/x.svg`.
   Relative paths break in `deck build`, which moves slides to
   `slides/<id>/index.html`.
5. **Run `deck check` before you call a slide done.** It catches overflow,
   clipped text, broken assets and console errors that are invisible until
   someone is presenting.

## File conventions

| | |
|---|---|
| Order | the file path, sorted lexicographically — `10-…` before `20-…` |
| Identity | `<deck-slide id="…">`, which URLs and the presenter view use |
| Title | `<title>`, shown in the presenter view and the index |

Numbers go up in tens so there is room to insert. Use the CLI rather than
inventing a number:

```bash
deck add slide security --after architecture   # becomes 25-security.html
```

Never rename a file to reorder it without checking who links to its id — the
id is independent of the number, so reordering is free.

## Slide skeleton

```html
<!doctype html>
<html lang="ja">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <title>Architecture</title>

  <link rel="stylesheet" href="/@deck/design.css">
  <script type="module" src="/@deck/boot.js"></script>
</head>
<body>
  <deck-slide id="architecture">
    <deck-heading eyebrow="Architecture">
      One pipeline, end to end
    </deck-heading>

    <deck-grid columns="2" class="grow">
      <deck-card data-step="1">
        <h2 class="text-body font-bold">Collection</h2>
        <p>Probes push measurements into the ingest tier.</p>
      </deck-card>

      <deck-card data-step="2" variant="accent">
        <h2 class="text-body font-bold">Visualization</h2>
        <p>Queries fan out to the visualisation layer.</p>
      </deck-card>
    </deck-grid>

    <deck-footer divider>
      <span>Deck title</span>
      <deck-slide-number></deck-slide-number>
    </deck-footer>

    <deck-notes>Explain the ingest path and the query path separately.</deck-notes>
  </deck-slide>
</body>
</html>
```

`deck-slide` is a flex column. `class="grow"` on the main block makes it take
the free space; `deck-footer` then sits at the bottom on its own.

## Built-in components

| Component | For |
|---|---|
| `deck-slide` | the slide root, one per document. `layout="title\|center\|bleed"` |
| `deck-heading` | heading with optional `eyebrow` and `sub` attributes |
| `deck-eyebrow` `deck-title` `deck-subtitle` | the same three parts as child elements |
| `deck-grid` | `columns="3"`, `gap="24"` |
| `deck-stack` | vertical (or `direction="row"`) with `gap` |
| `deck-card` | `variant="accent\|outline\|plain"` |
| `deck-callout` | `tone="info\|warning\|success\|danger"`, `label="Note"` |
| `deck-stat` | big number then caption; `countup` animates it from zero |
| `deck-figure` | image with `caption` |
| `deck-code` | `language="rust\|js\|python\|go\|bash\|toml\|json"`, `highlight-lines="3-5"` |
| `deck-footer` `deck-slide-number` `deck-progress` | running footer and position |
| `deck-notes` | speaker notes; never rendered on the slide itself |

Inside `deck-code`, escape the markup: `&lt;div&gt;`.

Everything except `deck-code` is Light DOM, so ordinary CSS selectors and
Tailwind utilities apply to the insides.

## Steps

`data-step="N"` reveals an element at step N. Steps are **absolute**, so any
step is directly addressable and a reload lands in the same place.

```html
<p data-step="1">Shown at step 1 and after.</p>
<p data-step="2">Shown at step 2 and after.</p>
```

Elements keep their space while hidden, so nothing jumps. For anything richer,
listen for the event:

```html
<script type="module">
  document.addEventListener("deck:stepchange", (event) => {
    if (event.detail.to === 3) { /* … */ }
  });
</script>
```

Also available: `deck:init`, `deck:ready`, `deck:enter`, `deck:leave`,
`deck:prepare-print`. `window.deck` exposes `mode`, `slideId`, `step`,
`stepCount`, `position` and `registerTimeline()`.

## Styling

Three layers, in increasing priority:

1. **Design tokens** — `design/tokens.css` for `--deck-*` values, and
   `design/theme.css` for the project look. Change a token to restyle the whole
   deck.
2. **Tailwind utilities** — usable on any element. Tokens are bridged into the
   Tailwind theme, so `bg-surface`, `text-accent`, `text-small`, `font-mono` and
   `rounded-card` follow the deck theme. Add `@theme`, `@utility` and `@apply`
   rules in `design/tailwind.css`.
3. **Slide-local CSS** — a `<style>` in the slide, wrapped in `@layer slide { … }`
   so it stays below Tailwind utilities.

Prefer editing a token over overriding a component, and prefer a utility class
over a one-off `<style>`.

## Anime.js

```html
<script type="module">
  import { animate, createTimeline } from "/@deck/vendor/animejs.js";

  const timeline = createTimeline();
  window.deck.registerTimeline(timeline); // printing seeks it to the end
</script>
```

Animation is skipped in print and check mode, so both stay deterministic. Never
rely on an animation to make content legible.

`deck-stat countup` is step-aware: it animates when the stat becomes visible
and re-arms when it is hidden again, so stepping back and forth replays it.
Combine it with `data-step` to control when it fires.

## Before finishing

```bash
deck check              # static + runtime + layout, exit code 1 on violations
deck check --slide architecture --screenshots
```

Fix violations rather than suppressing them. When a warning really is
intentional:

```html
<div data-deck-check-ignore="outside-safe-area">…</div>
```

Common failures and what they mean:

| Rule | Usually means |
|---|---|
| `slide_overflow` | too much content; cut text or split the slide |
| `clipped_text` | a fixed-height box is smaller than its text |
| `outside_safe_area` | content is closer than 56/64px to an edge |
| `min_font_size` | below 18px; unreadable from the back of a room |
| `low_contrast` | below WCAG AA against the actual background |
| `text_overlap` | two text blocks sit on top of each other |
| `missing_file` | a referenced asset does not exist, or the path is relative |

## Assets

```text
assets/images/   photographs, diagrams, screenshots
assets/icons/    small SVGs
assets/fonts/    webfonts, registered automatically from the file name
assets/data/     JSON, CSV, anything a slide fetches
assets/cursor.svg  optional custom mouse cursor
```

A font named `Inter-Bold-Italic.woff2` registers as Inter, weight 700, italic —
no `@font-face` to write. Then point a token at it:

```css
:root { --deck-font-sans: "Inter", sans-serif; }
```

## Commands

```bash
deck dev                 # dev server with hot reload
deck add slide <name> [--after <id>]
deck check [--static] [--changed] [--report json|sarif]
deck build               # static output in dist/
deck component list      # what is available
```
