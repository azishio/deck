# Built-in components

The design system ships as Custom Elements in the `deck-*` namespace. Everything except
`deck-code` renders in **Light DOM**, so ordinary CSS selectors, Tailwind utilities,
Anime.js and DevTools all keep working on the insides.

Content goes in **child elements**, not attributes. A JSON blob in an attribute is a
slide DSL in disguise; if you find yourself reaching for one, write plain HTML instead.

```bash
deck component list              # what is available, built-in and yours
deck component show deck-card    # the built-in styles for one component
```

## Structure

**`deck-slide`** — the slide root, one per document. `id` is the stable identity;
`layout` is `title`, `center` or `bleed`.

**`deck-grid`** — a CSS grid. `columns="3"`, `gap="24"` (px or any CSS length),
`align="start|center|stretch"`.

**`deck-stack`** — a flex column, or a row with `direction="row"`. `gap`, `align`, and
`grow` to take the free space.

```html
<deck-grid columns="2" gap="40" class="grow">
  <deck-stack gap="16"> … </deck-stack>
  <deck-figure caption="Ingest path">
    <img src="/assets/images/ingest.svg" alt="">
  </deck-figure>
</deck-grid>
```

## Headings

**`deck-heading`** — a heading with optional `eyebrow` and `sub` attributes, and
`level="title"` for the larger size.

**`deck-eyebrow` / `deck-title` / `deck-subtitle`** — the same three parts as child
elements. Use these when the text is content you want in the markup rather than
configuration squeezed into attributes:

```html
<deck-eyebrow>Summary</deck-eyebrow>
<deck-title>What we shipped</deck-title>
<deck-subtitle>Three quarters, one pipeline</deck-subtitle>
```

## Content blocks

**`deck-card`** — a bordered surface. `variant="accent|outline|plain"`.

**`deck-callout`** — an aside with a coloured edge. `tone="info|warning|success|danger"`
and `label="Note"`.

**`deck-stat`** — a big number followed by a caption. Add `countup` to animate it from
zero, and `countup-duration="1200"` to slow it down:

```html
<deck-stat countup>
  <span>1,280</span>
  <span>requests per second</span>
</deck-stat>
```

**`deck-figure`** — an image or SVG with a `caption`. The media is `object-fit: contain`,
so it never overflows.

**`deck-code`** — syntax-highlighted code, and the one component that uses Shadow DOM,
because the internal `<pre><code>` structure is not something a slide should style
directly. `language` accepts `rust`, `js`/`ts`, `python`, `go`, `bash`, `toml` and
`json`; `highlight-lines="3-5,8"` marks lines.

Escape the markup inside it: `&lt;div&gt;`. Style it through
`--deck-code-font-size` and the `part="pre"` / `part="code"` hooks.

## Running furniture

**`deck-footer`** — a bottom band. `divider` adds a rule. It is pushed down with
`margin-block-start: auto` rather than positioned absolutely, so it can never overlap
the content above it — give the block above it `class="grow"` and the footer takes care
of itself.

**`deck-slide-number`** — this slide's position. `format` is a template with `{number}`,
`{total}` and `{percent}`, defaulting to `{number} / {total}`.

**`deck-progress`** — a thin bar showing how far through the deck the slide is.

Both read the deck manifest, so the numbering survives inserting and reordering slides.

```html
<deck-footer divider>
  <span>ACME · 2026</span>
  <deck-slide-number></deck-slide-number>
</deck-footer>
```

## Speaker notes

**`deck-notes`** — never rendered on the slide. It appears in the presenter view, and in
the print view when `[print] show_notes = true`.
