# Visual and interactive slides

A slide here is a whole web page, not a text box on a background. That is the reason the
input format is HTML at all — if slides were only ever going to be headings and bullets, a
DSL would have been less typing.

So the question to ask of each slide is not "what should it say" but **what should the
audience watch happen**. Most of the time the best answer is an SVG scene that moves as
you step through it.

| Instead of | Build |
|---|---|
| "The pipeline has four stages" | stages that light up as a token rides through them |
| "Latency dropped 40%" | a chart that draws itself, the old line still visible |
| "The layers stack like this" | the stack, assembling one layer per step |
| "The algorithm backtracks here" | the walk, animated forwards and backwards |
| A bullet list of trade-offs | two quantities moving against each other |

The test is whether the drawing carries the argument. A diagram next to the same three
bullets is still three bullets.

Every slide in [the deck this guide ships with](https://azishio.github.io/deck/slide/) is
built this way, and its source is in [`site/slides/`](https://github.com/azishio/deck/tree/main/site/slides).

## The scene

Inline the SVG rather than referencing an `<img>`: it then inherits the deck's colours,
responds to `data-step`, and can be animated element by element.

```html
<svg class="scene" viewBox="0 0 1150 320" fill="none" role="img"
     aria-label="what a reader who cannot see it needs to know">
  <g class="node">
    <rect x="10" y="80" width="200" height="80" rx="10"/>
    <text x="110" y="114">Ingest</text>
  </g>
  <path id="route" d="M210 120 H 450"/>
</svg>
```

```css
@layer slide {
  /* Sized from the viewBox: stretching it to the free space scales the drawing
     up, and the first you hear of it is an outside_safe_area warning. */
  #my-slide .scene { width: 100%; height: auto; }

  /* Scaling or rotating an SVG element needs a box to transform about. */
  #my-slide .grows { transform-box: fill-box; transform-origin: center; }

  #my-slide .node rect { fill: var(--deck-color-surface); stroke: var(--deck-color-border); }
  #my-slide .node text { fill: var(--deck-color-text); font-size: 18px; text-anchor: middle; }
}
```

Habits that save time later:

- **Paint with tokens**, not hex, so a theme change carries.
- **Text in SVG is text.** The `min_font_size` and `low_contrast` rules read it, and a
  label that runs past its box is reported like any other overflow.
- **`data-step` works on SVG children.** The reveal is `opacity` and `visibility`, which
  apply to SVG elements exactly as they do to HTML.
- **`aria-hidden="true"`** on decorative drawings; `role="img"` and an `aria-label` on
  ones that carry meaning.

## Animating it

Anime.js v4 is vendored, so importing it needs no install and no network. Its SVG helpers
each want a different shape, and that is the part worth reading rather than guessing:

```js
import { animate, createTimeline, stagger, svg, utils } from "/@deck/vendor/animejs.js";

// Draw a stroke on, as if by hand.
animate(svg.createDrawable(path), { draw: ["0 0", "0 1"], duration: 600, ease: "outQuad" });

// Morph one shape into another — the VALUE of `d`, not spread.
animate(shape, { d: svg.morphTo("#target"), duration: 400 });

// Ride a path — spread, because it returns translateX/translateY/rotate.
animate(token, { ...svg.createMotionPath("#route"), duration: 900, ease: "inOutQuad" });

// Fan a set out in time.
animate(items, { opacity: [0, 1], scale: [0.94, 1], delay: stagger(70), duration: 320 });

// Per-target values: the function receives the element.
animate(items, { translateX: (item) => [offsetOf(item), 0] });

// Tween a number by animating a plain object.
const counter = { value: 0 };
animate(counter, {
  value: 7.91,
  duration: 620,
  onUpdate: () => { label.textContent = counter.value.toFixed(2); },
});

// Set state without animating: the branch for print, check and reduced motion.
utils.set(items, { opacity: 1, translateX: 0 });
```

`createTimeline()` sequences beats — consecutive `.add()` calls run one after another —
and anything animating an SVG **attribute** (`cx`, `width`, `d`) behaves like a CSS
property.

When a ride is meant to stop short, draw a second path rather than racing a timer against
a running animation:

```html
<path id="route-full" d="M110 120 H 1010"/>
<path id="route-gate" d="M110 120 H 550"/>   <!-- geometry only, never stroked -->
```

## Driving a scene from the step model

Two patterns. The choice is about how much state the scene has.

### Groups, for a scene that accumulates

Put `data-step` on a `<g>`: the runtime then handles visibility in every mode, including
print and check. Add the choreography with `deck.onReveal`, which fires when the group
becomes visible and aborts when it stops being — see [Animation](./animation.md) for why
that beats animating at load.

```js
window.deck.onReveal(wire, async ({ signal }) => {
  if (!(await window.deck.animator())) {
    return;                                   // print and check already show the end state
  }
  const timeline = createTimeline();
  timeline.add(svg.createDrawable(wire), { draw: ["0 0", "0 1"], duration: 380 });
  timeline.add(blocks, { opacity: [0, 1], scale: [0.94, 1], delay: stagger(90) });
  signal.addEventListener("abort", () => timeline.revert());
});
```

### One `apply(step)`, for a scene that moves

When each step is a different arrangement rather than one more thing on screen, keep a
single function that can draw any step, and call it from both directions:

```js
window.deck.setStepCount(3);          // nothing in the markup carries data-step

function apply(step, { animated }) {
  const state = STATES[Math.min(step, STATES.length - 1)];
  label.textContent = state.caption;
  if (!animated) {
    utils.set(marker, { cx: state.x });
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

Three details there are not decoration:

- **`onReveal` applies the current step as well.** A deep link applies its step before this
  module exists, so listening only for `deck:stepchange` leaves the scene blank at
  `#/slide/2`.
- **`setStepCount` is required** when nothing carries `data-step`; without it the slide has
  no steps and the first arrow key leaves it. `deck check` knows about the declaration, so
  it does not report the disagreement with the markup.
- **`event.detail.instant`** is set when the step was arrived at rather than walked to — a
  catch-up on connect, a reload, a jump from the presenter view. Animating then replays
  reveals nobody asked for.

## When the audience should drive

Some things are worth handing over: a parameter with a surprising shape, a trade-off that
is easier felt than described. Navigation lives inside the slide document — a click on the
right half advances, arrow keys step — so the runtime hands input back to anything that
plainly wants it: `input`, `select`, `textarea`, `button`, links, `details`,
`[contenteditable]`, `[tabindex]`, and elements carrying an interactive `role`.

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
silently kill click navigation, and a presenter clicking with nothing happening is a worse
failure than a stray page turn.

Render the resting state immediately, so the slide reads before anyone touches it and the
printed copy is not blank, and reset the control from `deck.onReveal` so re-entering the
slide starts from a known state rather than wherever the last person left it.

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

## Checking a scene

`deck check` applies to a drawing as much as to prose: `slide_overflow` when the SVG is
taller than the canvas, `min_font_size` and `low_contrast` on SVG text, `missing_file` on
a generated image that was never written.

A demo that is *meant* to fail a rule — a contrast box you drive into the red — should say
so on the element rather than lower the rule for the whole deck:

```html
<g data-deck-check-ignore="low_contrast min_font_size">…</g>
```

Then watch it move, which is the part a check cannot do:

```bash
deck dev --port 5173 --open none
```

```js
document.documentElement.dataset.deckReady === "true";   // wait for this first
window.deck.next();                                      // walk it, do not jump
document.querySelector("#token").style.transform;        // did the ride happen?
window.deck.goToStep(0);
window.deck.next();                                      // does it replay?
```

One caveat worth knowing: `requestAnimationFrame` does not fire in a document that is not
being composited, so an animation cannot be observed in a hidden or background window. If
an animation appears not to run, check that before suspecting the code.

Finally, add `?deck-mode=check` to freeze animation and confirm the still frame still
makes the point. That is what the printed deck, and the person reading from the back,
actually get.
