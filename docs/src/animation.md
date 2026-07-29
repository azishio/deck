# Animation

Two layers, deliberately separated: the **shell** owns transitions between slides, and
each **slide** owns what happens inside it.

## The standard reveal

`data-step` is enough for most slides. The runtime fades and lifts each element as it is
revealed, with a small stagger, driven by tokens:

```css
:root {
  --deck-step-duration: 420ms;
  --deck-step-distance: 12px;
}
```

Nothing to write, and it is consistent across the deck. Reach for the rest of this page
only when a reveal is not enough.

## Anime.js

Anime.js v4 is vendored, so importing it needs no install and no network:

```html
<script type="module">
  import { animate, createTimeline, stagger } from "/@deck/vendor/animejs.js";

  const timeline = createTimeline();
  window.deck.registerTimeline(timeline);   // printing seeks it to its final frame
</script>
```

`registerTimeline` matters for print: `/print` drives each slide to its requested step
and then seeks every registered timeline to the end, so a printed slide shows the final
state rather than frame one.

## Animate on reveal, never on construction

This is the one rule worth internalising.

An element is constructed when its **iframe** is created, and the shell preloads the
neighbouring slides. So an animation started from `connectedCallback` runs while the
slide is still off-screen, finishes before you arrive, and then appears to re-trigger at
random depending on how far away the previous slide was.

`deck.onReveal` ties the animation to visibility instead:

```js
window.deck.onReveal(element, ({ signal, step }) => {
  const animation = animate(element, { opacity: [0, 1], translateY: [12, 0] });
  signal.addEventListener("abort", () => animation.revert());
});
```

It fires when the slide is entered **and** the element's `data-step` threshold is
reached, and the `AbortSignal` aborts as soon as either stops being true. So the
animation cannot run off-screen, cannot outlive its reveal, and replays when you step
back and forward again — the same behaviour as the standard reveal.

`deck-stat`'s `countup` is built on it, and reflects its state in
`data-deck-countup` (`idle`, `running`, `done`) if you want to hook CSS onto it.

## Reduced motion, print and check

Animation is skipped entirely in **print** and **check** mode, so screenshots, printed
pages and layout checks are deterministic. `[animation] reduced_motion` decides what the
OS preference does:

| | |
|---|---|
| `instant` *(default)* | honour it, and jump to the final state |
| `respect` | honour it |
| `ignore` | always animate |

`deck.animator()` returns `null` whenever animation is off, so the same code path handles
all of it:

```js
const animate = await window.deck.animator();
if (!animate) {
  element.textContent = finalValue;   // final state, no animation
  return;
}
```

## The runtime API

`window.deck` inside a slide:

| | |
|---|---|
| `mode` | `present`, `presenter`, `print`, `check` or `standalone` |
| `slideId` `step` `stepCount` | where you are |
| `position` `whenPositioned()` | `{ index, number, total }` from the manifest |
| `canvas` | `{ width, height }` |
| `reducedMotion` `ready` | flags |
| `onReveal(element, cb)` | run `cb` when the element becomes visible |
| `animator()` `anime()` | Anime.js access, or `null` when animation is off |
| `registerTimeline(t)` | seek `t` to the end before printing |
| `waitUntil(promise)` | delay `deck:ready` until it settles |
| `setStepCount(n)` | declare steps not expressed with `data-step` |
| `advance(dir)` `goToStep(n)` `goToSlide(id)` | navigate |

Events on `document`:

| Event | When |
|---|---|
| `deck:init` | initial state is known; `waitUntil()` available |
| `deck:ready` | fonts, images, components and custom promises have settled |
| `deck:enter` / `deck:leave` | the slide became / stopped being current |
| `deck:stepchange` | a step was applied (`from`, `to`, `direction`, `instant`) |
| `deck:pause` / `deck:resume` | the presenter paused or resumed |
| `deck:prepare-print` | finalise state before printing; `waitUntil()` available |
| `deck:dispose` | the iframe is about to go away |

Prefer `onReveal()` over `deck:enter` for animation: `deck:enter` fires once per visit and
knows nothing about steps.

## Slide transitions

Transitions between slides belong to the shell, not to a slide. It crossfades frames; a
hot reload uses the same crossfade between the old and new iframe, which is why editing
the current slide never flashes white.
