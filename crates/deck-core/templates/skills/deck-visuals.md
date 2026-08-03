---
name: deck-visuals
description: Explain something visually or interactively on a slide. Use when a slide would be clearer as a diagram, an animation or something the audience can operate — drawing inline SVG, animating it with Anime.js, morphing or drawing paths, moving along a motion path, using canvas, adding sliders, buttons or drag interaction, or generating artwork into assets/images/.
---

# Visual and interactive slides

A slide here is a whole web page, not a text box on a background. Anything a
browser can do, a slide can do: SVG, canvas, Web Animations, drag, live
controls, a running simulation. Sibling skills: **deck-slides** for the markup
and step model, **deck-components** for packaging behaviour as an element,
**deck-styling** for the look. Full guide:
<https://azishio.github.io/deck/visuals.html>.

## Reach for a picture first

Three cards of prose is the default a slide tool pushes you into. It is rarely
the clearest thing available here, and it is never the only one.

| Instead of | Build |
|---|---|
| "The pipeline has four stages" | an SVG pipeline whose stages light up per step |
| "Latency dropped 40%" | a chart that draws itself, with the before line still visible |
| A screenshot of a form | the actual form, working, in the slide |
| "The algorithm backtracks here" | a stepper the presenter can drive back and forth |
| A bullet list of trade-offs | two sliders and a readout that moves as you drag |
| A stock photo | nothing, or an SVG you drew for this exact point |

The test is whether the visual *carries the argument*. A decorative graphic
beside the same three bullets is still three bullets.

## Inline SVG is the default medium

Inline it — never `<img src>` — so it inherits the deck's colours, responds to
`data-step`, and can be animated element by element.

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

Rules that save time later:

- **Use `viewBox` and size with CSS**, never hard-coded `width`/`height` in
  pixels on the root — the canvas is fixed at 1280×720 and the slide scales.
- **Paint with tokens** (`var(--deck-color-accent)`), so a theme change carries.
- **`data-step` works on SVG children.** The reveal is `opacity`/`visibility`,
  which applies to SVG elements exactly as it does to HTML.
- **Text in SVG is text.** `deck check`'s contrast and font-size rules read it,
  so keep it at 18px or more in `viewBox` units that end up ≥18 CSS px.
- **`aria-hidden="true"`** on decorative SVG; a `<title>` element on meaningful
  ones.

## Animating it

Anime.js is vendored — no install, no network. Its SVG helpers are the reason to
prefer SVG over an image.

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

**Always start an animation from `deck.onReveal`, never at load.** The shell
preloads neighbouring slides, so an animation started on construction runs
off-screen and is over before anyone sees it.

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

`onReveal` fires when the slide is entered **and** the element's `data-step`
threshold is reached, and aborts when either stops holding — so it replays when
you step back and forward, and never runs where nobody is looking.

For a long sequence, register the timeline so printing seeks it to the end:

```js
window.deck.registerTimeline(timeline);
```

And handle animation being off — reduced motion, printing, `deck check`:

```js
const anime = await window.deck.animator();
if (!anime) {
  drawFinalState();   // the same picture, arrived at instantly
  return;
}
```

CSS animation and SMIL work too. Prefer CSS for something small and looping (a
pulsing dot), Anime.js when the deck's step model has to drive it.

## Interaction

The runtime navigates on clicks and arrow keys, and it hands both back to
anything that plainly wants them: `input`, `select`, `textarea`, `button`,
links, `details`, `[contenteditable]`, `[tabindex]`, and elements carrying an
interactive `role`. A focused text field or slider keeps every key; a focused
button keeps Space and Enter, so arrows still move the deck.

Anything else that is operable — a `<canvas>` you drag on, an SVG hotspot, a
widget of your own — needs to say so, because a full-slide decorative canvas
that ate every click would be worse:

```html
<div data-deck-no-nav>
  <canvas id="field" width="720" height="360"></canvas>
</div>
```

`data-deck-no-nav` covers the whole subtree, for clicks and for keys. Put it on
`deck-slide` itself for a slide that is entirely a widget — then navigate with
the presenter view, or leave a button that calls `window.deck.advance()`.

A worked control:

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

Two things make a live control work in front of an audience:

- **Render the resting state immediately**, so the slide is readable before
  anyone touches it and so a printed copy is not blank.
- **Keep it operable with one gesture.** A presenter has one hand and a few
  seconds; anything needing precision will not survive the room.

`deck.onReveal` is the place to reset a widget, so re-entering the slide starts
from a known state rather than wherever the last person left it.

## Canvas and generated artwork

Canvas suits particle fields, plots of many points, and anything simulated. Size
the backing store to the layout and draw on reveal:

```js
window.deck.onReveal(canvas, ({ signal }) => {
  const context = canvas.getContext("2d");
  let frame;
  const tick = () => { draw(context); frame = requestAnimationFrame(tick); };
  tick();
  signal.addEventListener("abort", () => cancelAnimationFrame(frame));
});
```

Canvas is invisible to the layout checks and to text extraction, so put anything
that has to be read — labels, legends, numbers — in HTML or SVG on top of it.

For artwork, **generate it rather than going looking for one**. An SVG you write
scales, themes and animates; a stock photo does none of that. When you do
produce raster output — from an image model or a plotting script — write it to
`assets/images/` and reference it root-absolutely:

```html
<deck-figure caption="Sampled trajectories">
  <img src="/assets/images/trajectories.png" alt="Sampled trajectories" width="720" height="360">
</deck-figure>
```

Give every `<img>` a `width` and `height` so the layout does not jump, and keep
the source (the script, the prompt) next to the output if it will ever need
regenerating.

## Checking

```bash
deck check
```

The rules apply to a drawing as much as to prose: `slide_overflow` when the SVG
is taller than the canvas, `min_font_size` and `low_contrast` on SVG text,
`missing_file` on a generated image that was never written.

Then drive it, which is the part a check cannot do:

```bash
deck dev --port 5173 --open none
# http://127.0.0.1:5173/slides/pipeline
```

```js
document.documentElement.dataset.deckReady === "true";   // wait for this
window.deck.goToStep(0);
window.deck.goToStep(2);                                 // does the reveal replay?
document.querySelector("#threads").value = "12";
document.querySelector("#threads").dispatchEvent(new Event("input"));
document.querySelector("#throughput").textContent;
```

Add `?deck-mode=check` to freeze animation for a deterministic screenshot, and
confirm the still image still makes the point — that is what the printed deck
and the slow reader get.
