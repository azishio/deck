---
name: deck-visuals
description: Explain something with a drawing that moves. Use when a slide would be clearer as a diagram or an animation — building an inline SVG scene, animating it with Anime.js, drawing strokes on, morphing shapes, riding a motion path, staggering a set, tweening a number, driving a scene from the step model, or generating artwork into assets/images/.
---

# Visual slides

A slide here is a whole web page, so the honest answer to "how do I explain
this" is usually **an SVG scene that animates as you step through it** — not a
paragraph, and not a widget with buttons on it. Sibling skills: **deck-slides**
for the markup and step model, **deck-components** for packaging behaviour as an
element, **deck-styling** for the look. Full guide:
<https://azishio.github.io/deck/visuals.html>.

## Draw the argument

| Instead of | Build |
|---|---|
| "The pipeline has four stages" | stages that light up as a token rides through them |
| "Latency dropped 40%" | a chart that draws itself, the old line still visible |
| "The layers stack like this" | the stack, assembling one layer per step |
| "The algorithm backtracks here" | the walk, animated forwards and backwards |
| A bullet list of trade-offs | two quantities moving against each other |

The test is whether the drawing **carries the argument**. A diagram next to the
same three bullets is still three bullets.

Reach for HTML controls — sliders, buttons, text fields — only when the audience
genuinely has to drive something. A scene that plays out per step reads better
from the back of a room and prints correctly.

## The scene

Inline the SVG. It then inherits the deck's colours, responds to `data-step`, and
can be animated element by element.

```html
<svg class="scene" viewBox="0 0 1150 320" fill="none" role="img"
     aria-label="what a reader who cannot see it needs to know">
  <g class="node" data-node="ingest">
    <rect x="10" y="80" width="200" height="80" rx="10"/>
    <text x="110" y="114">Ingest</text>
  </g>
  <path id="route" d="M210 120 H 450"/>
</svg>
```

```css
@layer slide {
  /* Sized from the viewBox: stretching it to the free space scales the drawing
     up and pushes it past the safe area. */
  #my-slide .scene { width: 100%; height: auto; }

  /* Scaling or rotating an SVG element needs a box to transform about. */
  #my-slide .grows { transform-box: fill-box; transform-origin: center; }

  #my-slide .node rect { fill: var(--deck-color-surface); stroke: var(--deck-color-border); }
  #my-slide .node text { fill: var(--deck-color-text); font-size: 18px; text-anchor: middle; }
}
```

Rules that save time:

- **Paint with tokens**, never hex, so a theme change carries.
- **Text in SVG is text.** `min_font_size` and `low_contrast` read it; keep it at
  18px in units that end up ≥18 CSS px, and keep labels inside their boxes —
  a label overflowing its group is reported like any other overflow.
- **`aria-hidden="true"`** on decorative drawings, `role="img"` plus an
  `aria-label` on ones that carry meaning.

## Animating it

Anime.js is vendored — no install, no network:

```js
import { animate, createTimeline, stagger, svg, utils } from "/@deck/vendor/animejs.js";
```

The SVG helpers each want a different shape. This is the part that wastes an
hour if you guess:

```js
// Draw a stroke on, as if by hand.
animate(svg.createDrawable(path), { draw: ["0 0", "0 1"], duration: 600, ease: "outQuad" });

// Morph one shape into another — the VALUE of `d`, not spread.
animate(shape, { d: svg.morphTo("#target"), duration: 400 });

// Ride a path — spread, because it returns translateX/translateY/rotate.
animate(token, { ...svg.createMotionPath("#route"), duration: 900, ease: "inOutQuad" });

// Fan a set out in time.
animate(items, { opacity: [0, 1], scale: [0.94, 1], delay: stagger(70), duration: 320 });

// Per-target values: a function receives the element.
animate(items, { translateX: (item) => [offsetOf(item), 0] });

// Tween a number by animating a plain object.
const counter = { value: 0 };
animate(counter, {
  value: 7.91,
  duration: 620,
  onUpdate: () => { label.textContent = counter.value.toFixed(2); },
});

// Set state without animating — the branch for print, check and reduced motion.
utils.set(items, { opacity: 1, translateX: 0 });
```

`createTimeline()` sequences beats: consecutive `.add()` calls run one after the
other. Anything animating an SVG **attribute** (`cx`, `width`, `d`) works the
same as a CSS property.

**Stop where the story stops by drawing a second path**, rather than racing a
timer against a running animation:

```html
<path id="route-full" d="M110 120 H 1010"/>
<path id="route-gate" d="M110 120 H 550"/>   <!-- geometry only, never stroked -->
```

## Driving a scene from steps

Two patterns, and the choice is about how much state the scene has.

**Groups, for a scene that only accumulates.** Put `data-step` on a `<g>` and
the runtime handles visibility in every mode, including print and check. Add the
choreography with `onReveal`, which fires when the group actually becomes
visible and aborts when it stops being:

```js
window.deck.onReveal(wire, async ({ signal }) => {
  if (!(await window.deck.animator())) {
    return;                                   // print/check already show the end state
  }
  const timeline = createTimeline();
  timeline.add(svg.createDrawable(wire), { draw: ["0 0", "0 1"], duration: 380 });
  timeline.add(blocks, { opacity: [0, 1], scale: [0.94, 1], delay: stagger(90) });
  signal.addEventListener("abort", () => timeline.revert());
});
```

**A single `apply(step)`, for a scene with a state per step** — something that
moves rather than merely appears:

```js
window.deck.setStepCount(3);          // no [data-step] in the markup to count

function apply(step, { animated }) {
  const state = STATES[Math.min(step, STATES.length - 1)];
  label.textContent = state.caption;
  if (!animated) {
    utils.set(marker, { cx: state.x });       // print, check, reduced motion
    return;
  }
  animate(marker, { cx: state.x, duration: 360, ease: "outQuad" });
}

window.deck.onReveal(scene, async () => apply(window.deck.step, { animated: false }));

document.addEventListener("deck:stepchange", async (event) => {
  apply(event.detail.to, {
    animated: Boolean(await window.deck.animator()) && !event.detail.instant,
  });
});
```

Three details in there are not decoration:

- **`onReveal` applies the current step too.** A deep link applies its step
  before this module exists, so listening for `deck:stepchange` alone leaves the
  scene blank at `#/slide/2`.
- **`setStepCount`** is required when nothing carries `data-step`; without it the
  slide has no steps and the first arrow key leaves it. `deck check` knows about
  the declaration and will not report the disagreement.
- **`event.detail.instant`** is set when the step was not walked to — a catch-up
  on connect, a reload. Animating then replays reveals nobody asked for.

Never start an animation at load. The shell preloads neighbouring slides, so it
would run off-screen and be over before anyone arrives.

## When the audience should drive

Some things are worth handing over — a parameter with a surprising shape, a
counter-intuitive trade-off. Form controls, links, buttons and anything focusable
keep their own clicks and keys; a focused slider keeps every key, a focused
button keeps only Space and Enter so the arrows still move the deck. Anything
else operable, such as a canvas you drag on, needs `data-deck-no-nav` — which
covers its whole subtree, for clicks and keys.

Render the resting state immediately so the slide reads before anyone touches
it, and reset it from `onReveal` so re-entering starts from a known state.

## Canvas and generated artwork

Canvas suits particle fields, many-point plots and simulations. Draw on reveal,
stop on abort, and keep anything that has to be **read** in SVG or HTML on top —
canvas is invisible to the layout checks and to text extraction.

For artwork, generate it rather than going looking. An SVG you write scales,
themes and animates; a stock photo does none of that. Raster output from an image
model or a plotting script goes in `assets/images/`, referenced root-absolutely
with `width` and `height` so the layout does not jump.

## Checking

```bash
deck check
```

The rules apply to a drawing as much as to prose. A demo that is *meant* to fail
one — a contrast box you drive into the red — says so on the element rather than
lowering the rule for the deck:

```html
<g data-deck-check-ignore="low_contrast min_font_size">…</g>
```

Then watch it move, which a check cannot do:

```bash
deck dev --port 5173 --open none
# http://127.0.0.1:5173/slides/pipeline
```

```js
document.documentElement.dataset.deckReady === "true";   // wait for this first
window.deck.next();                                      // walk it, do not jump
document.querySelector("#token").style.transform;        // did the ride happen?
window.deck.goToStep(0);
window.deck.next();                                      // does it replay?
```

`requestAnimationFrame` does not fire in a document that is not being
composited, so an animation cannot be observed in a hidden or background window.
Check that before suspecting the code.

Finally, `?deck-mode=check` freezes animation: confirm the still frame still
makes the point, because that is what the printed deck gets.
