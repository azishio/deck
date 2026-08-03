# Writing a slide

A slide is a complete HTML document. Nothing is generated from a template at build time,
so what you read in the file is what the browser gets.

This page covers the parts every slide has. What goes in the middle of one is a separate
question, and usually a more interesting one — see
[Visual and interactive slides](./visuals.md).

```html
{{#include ../snippets/slide.html}}
```

Four things to notice:

1. **The two runtime tags.** `/@deck/design.css` brings in the design system, and
   `/@deck/boot.js` starts the slide runtime. If you leave them out, the server injects
   them — but writing them keeps the file honest about its dependencies.
2. **`<deck-slide id="…">` is the slide root.** One per document. Its `id` is the slide's
   stable identity.
3. **`<title>` is the display title**, shown in the presenter view and the index.
4. **`<deck-notes>` never renders on the slide.** It is speaker-only.

## The canvas is fixed

Every slide is laid out on exactly **1280×720 CSS pixels** (configurable, but constant
across the deck). The presentation shell scales the whole iframe to fit the screen.

This means: **do not write responsive layout.** No `vw`, no `vh`, no media queries. Use
absolute sizes and flexbox, and let the shell handle screens. A slide that reflows at
different widths will look right on your laptop and wrong on the projector.

`deck-slide` is a flex column with the deck's padding applied. `layout` picks a variant:

| `layout` | Effect |
|---|---|
| *(none)* | content stacked from the top |
| `title` | vertically centred, tighter gaps |
| `center` | vertically centred |
| `bleed` | no padding, for full-bleed imagery |

## Steps and reveals

`data-step="N"` reveals an element at step *N* and keeps it visible afterwards:

```html
<p data-step="1">Shown at step 1 and after.</p>
<p data-step="2">Shown at step 2 and after.</p>
```

Hidden elements keep their space, so revealing one never shifts the layout. The step
count is simply the highest `data-step` in the document.

Steps are **absolute**, not a sequence of "next" commands. The screen is a function of
`(slide id, step)`, which is why a deep link, a reload and a hot swap all land in exactly
the same state. The URL carries it: `/present#/architecture/2`.

For anything richer than a reveal, listen for the change:

```html
<script type="module">
  document.addEventListener("deck:stepchange", (event) => {
    const { from, to, direction, instant } = event.detail;
    if (to === 3) {
      // …
    }
  });
</script>
```

## Navigating

Every page navigates the same way, including a single slide opened on its own:

| | |
|---|---|
| `→` `Space` `PageDown`, or a click on the **right half** | next step, then the next slide |
| `←` `PageUp`, right-click, or a click on the **left half** | previous step, then the previous slide |
| `↑` `↓` | previous / next slide, skipping steps |
| `Home` `End` | first / last step |
| `b` | blackout |
| `f` | fullscreen |

Clicks on links, buttons, form controls and media are left alone, as is a click that
ends a text selection. Mark anything else that must not advance the deck with
`data-deck-no-nav`.

## Adding slides

Let the CLI pick the number, so gaps are used rather than guessed:

```bash
deck add slide security --after architecture   # -> slides/25-security.html
deck add slide summary                         # -> appended, next multiple of ten
```
