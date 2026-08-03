# Visual and interactive slides

A slide here is a whole web page, not a text box on a background. That is the reason the
input format is HTML at all — if slides were only ever going to be headings and bullets,
a DSL would have been less typing.

So the question to ask of each slide is not "what should it say" but **what should the
audience see happen**. If the answer is a structure, a change over time, a quantity or a
trade-off, the middle of the slide should be a drawing or a control rather than a
paragraph.

| Instead of | Build |
|---|---|
| "The pipeline has four stages" | an SVG pipeline whose stages light up per step |
| "Latency dropped 40%" | a chart that draws itself, with the old line still visible |
| A screenshot of a form | the actual form, working, in the slide |
| "The algorithm backtracks here" | a stepper the presenter can drive back and forth |
| A bullet list of trade-offs | two sliders and a readout that moves as you drag |

The test is whether the visual carries the argument. A diagram next to the same three
bullets is still three bullets.

Every slide in [the deck this guide ships with](https://azishio.github.io/deck/slide/) is
built this way, and its source is in `site/slides/` in the repository.

## Inline SVG is the default medium

Inline the SVG rather than referencing an `<img>`: it then inherits the deck's colours,
responds to `data-step`, and can be animated element by element.

```html
<svg viewBox="0 0 720 320" class="w-full" fill="none" aria-hidden="true">
  <rect x="16" y="120" width="160" height="80" rx="8"
        fill="var(--deck-color-surface)" stroke="var(--deck-color-border)"/>
  <text x="96" y="166" text-anchor="middle" font-size="18"
        fill="var(--deck-color-text)">Ingest</text>

  <g data-step="2">
    <path id="flow" d="M176 160 H 320" stroke="var(--deck-color-accent)" stroke-width="3"/>
  </g>
</svg>
```

A few habits save time later:

- **Size from the `viewBox`.** Give the root `width: 100%; height: auto` and let the
  aspect ratio decide the height. Stretching it to the free space instead scales the
  drawing up, and the first thing you hear about it is an `outside_safe_area` warning.
- **Paint with tokens** — `var(--deck-color-accent)` — so a theme change carries.
- **`data-step` works on SVG children.** The reveal is `opacity` and `visibility`, which
  apply to SVG elements exactly as they do to HTML.
- **Text in SVG is text.** The `min_font_size` and `low_contrast` rules read it, and a
  label that runs past its box will be reported like any other overflow.
- **`aria-hidden="true"`** on decorative drawings, a `<title>` element on meaningful ones.

## Animating a drawing

Anime.js is vendored, and its SVG helpers are most of the reason to prefer SVG over an
image:

```js
import { animate, createTimeline, stagger, svg } from "/@deck/vendor/animejs.js";
```

| Helper | Does |
|---|---|
| `svg.createDrawable(path)` | draw a stroke on, as if by hand |
| `svg.morphTo(target)` | morph one shape into another |
| `svg.createMotionPath(path)` | move something along a path |
| `stagger(80)` | fan a set of elements out in time |
| `createTimeline()` | sequence the whole explanation |

Start it from `deck.onReveal`, never at load — see
[Animation](./animation.md) for why that distinction matters more than it looks:

```html
<script type="module">
  import { animate, svg } from "/@deck/vendor/animejs.js";

  const flow = document.querySelector("#flow");

  window.deck.onReveal(flow, ({ signal }) => {
    const drawing = animate(svg.createDrawable(flow), {
      draw: ["0 0", "0 1"],
      duration: 700,
      ease: "outQuad",
    });
    signal.addEventListener("abort", () => drawing.revert());
  });
</script>
```

## Interaction

Navigation lives inside the slide document — a click on the right half advances, arrow
keys step — so an interactive slide and the deck are competing for the same input. The
runtime settles that by handing input back to anything that plainly wants it: `input`,
`select`, `textarea`, `button`, links, `details`, `[contenteditable]`, `[tabindex]`, and
elements carrying an interactive `role`.

The split is by key, not just by element. A focused text field or slider keeps **every**
key, because any of them could change its value. A focused button keeps only Space and
Enter, so the arrow keys still move the deck.

Anything else that is operable has to say so:

```html
<div data-deck-no-nav>
  <canvas id="field" width="720" height="360"></canvas>
</div>
```

`data-deck-no-nav` covers the whole subtree, for clicks and for keys. `canvas` and `svg`
are not on the automatic list on purpose: a decorative one covering the slide would
silently kill click navigation, and a presenter clicking with nothing happening is a
worse failure than a stray page turn.

For a slide that is entirely a widget, put the attribute on `deck-slide` itself and
navigate from the presenter view, or leave a control that calls `deck.advance()`.

```html
<label class="flex items-center gap-3">
  Threads
  <input id="threads" type="range" min="1" max="16" value="4">
  <output id="throughput" class="font-mono"></output>
</label>

<script type="module">
  const threads = document.querySelector("#threads");
  const throughput = document.querySelector("#throughput");
  const render = () => {
    const n = Number(threads.value);
    throughput.textContent = `${(n / (1 + 0.06 * n * n)).toFixed(1)}x`;
  };
  threads.addEventListener("input", render);
  render();
</script>
```

Two things make a live control survive a real room:

- **Render the resting state immediately**, so the slide reads before anyone touches it —
  and so the printed copy is not blank.
- **Keep it operable with one gesture.** A presenter has one hand and a few seconds.

`deck.onReveal` is the place to reset a widget, so re-entering a slide starts from a known
state rather than wherever the last person left it.

## Canvas and generated artwork

Canvas suits particle fields, plots of many points and anything simulated. Draw on reveal
and stop on abort:

```js
window.deck.onReveal(canvas, ({ signal }) => {
  const context = canvas.getContext("2d");
  let frame;
  const tick = () => { draw(context); frame = requestAnimationFrame(tick); };
  tick();
  signal.addEventListener("abort", () => cancelAnimationFrame(frame));
});
```

Canvas is invisible to the layout checks and to text extraction, so anything that has to
be read — labels, legends, numbers — belongs in HTML or SVG on top of it.

For artwork, prefer generating it to going looking for one. An SVG you write scales,
themes and animates; a stock photo does none of that. When you do produce raster output,
from an image model or a plotting script, write it to `assets/images/` and reference it
root-absolutely, with a `width` and `height` so the layout does not jump:

```html
<deck-figure caption="Sampled trajectories">
  <img src="/assets/images/trajectories.png" alt="Sampled trajectories"
       width="720" height="360">
</deck-figure>
```

Keep whatever produced it — the script, the prompt — next to the output, because the one
certainty is that it will need regenerating.

## Checking an interactive slide

`deck check` applies to a drawing as much as to prose: `slide_overflow` when the SVG is
taller than the canvas, `min_font_size` and `low_contrast` on SVG text, `missing_file` on
a generated image that was never written.

A demo that is *meant* to fail a rule — a contrast box you can drag into the red — should
say so on the element rather than lower the rule for the whole deck:

```html
<div data-deck-check-ignore="low_contrast min_font_size">…</div>
```

Then drive it, which is the part a check cannot do:

```bash
deck dev --port 5173 --open none
```

```js
document.documentElement.dataset.deckReady === "true";   // wait for this first
window.deck.goToStep(0);
window.deck.goToStep(2);                                 // does the reveal replay?
document.querySelector("#threads").value = "12";
document.querySelector("#threads").dispatchEvent(new Event("input"));
document.querySelector("#throughput").textContent;
```

One caveat worth knowing: `requestAnimationFrame` does not fire in a document that is not
being composited, so an animation cannot be observed in a hidden or background window. If
an animation appears not to run, check that before suspecting the code.

Finally, add `?deck-mode=check` to freeze animation and confirm the still image still
makes the point. That is what the printed deck, and the person reading from the back,
actually get.
