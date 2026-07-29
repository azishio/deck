---
name: deck-slides
description: Write and edit the slides of this deck. Use when adding a slide, rewriting one under slides/, reordering or renumbering slides, working with data-step reveals or speaker notes, or fixing a `deck check` layout violation. For colours, fonts and themes use deck-styling; for Custom Elements use deck-components.
---

# Writing slides in this deck

This project is a **deck**: a slide deck where one slide is one complete HTML
document. There is no slide DSL and no build step. Edit HTML directly.

Sibling skills: **deck-styling** for the look (tokens, themes, Tailwind, fonts),
**deck-components** for Custom Elements. The full guide is at
<https://azishio.github.io/deck/>.

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

Because identity is independent of order, renumbering never breaks a deep link.
Write an explicit `id` on any slide worth linking to.

## Slide skeleton

```html
<!doctype html>
<html lang="en">
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

## Components available

| Component | For |
|---|---|
| `deck-slide` | the slide root, one per document. `layout="title\|center\|bleed"` |
| `deck-heading` | heading with optional `eyebrow` and `sub` attributes |
| `deck-eyebrow` `deck-title` `deck-subtitle` | the same three parts as child elements |
| `deck-grid` | `columns="3"`, `gap="24"` |
| `deck-stack` | vertical (or `direction="row"`) with `gap`, `align`, `grow` |
| `deck-card` | `variant="accent\|outline\|plain"` |
| `deck-callout` | `tone="info\|warning\|success\|danger"`, `label="Note"` |
| `deck-stat` | big number then caption; `countup` animates it from zero |
| `deck-figure` | image with `caption` |
| `deck-code` | `language="rust\|js\|python\|go\|bash\|toml\|json"`, `highlight-lines="3-5"` |
| `deck-footer` `deck-slide-number` `deck-progress` | running footer and position |
| `deck-notes` | speaker notes; never rendered on the slide itself |

Inside `deck-code`, escape the markup: `&lt;div&gt;`. Run `deck component list`
to see project components too.

## Steps

`data-step="N"` reveals an element at step N. Steps are **absolute**, so any
step is directly addressable and a reload lands in the same place.

```html
<p data-step="1">Shown at step 1 and after.</p>
<p data-step="2">Shown at step 2 and after.</p>
```

Elements keep their space while hidden, so nothing jumps.

Any page navigates the same way — arrow keys, or a click on the left or right
half — including a single slide opened on its own, which carries on to the
adjacent slide once its steps run out. Put `data-deck-no-nav` on anything
clickable that must not advance the deck.

For anything richer, listen for the event:

```html
<script type="module">
  document.addEventListener("deck:stepchange", (event) => {
    if (event.detail.to === 3) { /* … */ }
  });
</script>
```

Also available: `deck:init`, `deck:ready`, `deck:enter`, `deck:leave`,
`deck:prepare-print`. `window.deck` exposes `mode`, `slideId`, `step`,
`stepCount`, `position` and `onReveal()`. For animation see **deck-components**;
never start one from `connectedCallback`.

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

## Commands

```bash
deck dev                 # dev server with hot reload
deck add slide <name> [--after <id>]
deck check [--static] [--changed] [--report json|sarif]
deck build               # static output in dist/
```
