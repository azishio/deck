---
name: deck-components
description: Add or edit a Custom Element for this deck. Use when creating a component under components/, registering one in components/index.js, writing element behaviour or animation inside a slide, deciding between Light DOM and Shadow DOM, using deck.onReveal or Anime.js, or fixing an invalid_component_name or undefined_component check.
---

# Components in this deck

A component is a Custom Element. There is no registry to declare, no build step
and no framework — if `customElements.define()` runs, the element works in every
slide. Sibling skills: **deck-slides** for slide markup, **deck-styling** for
the look. Full guide: <https://azishio.github.io/deck/own-components.html>.

## Scaffold one

```bash
deck component new acme-metric     # writes components/acme-metric.js and registers it
deck component list                # built-ins plus every tag under components/
deck component show deck-card      # a built-in's styles, worth reading first
```

`components/index.js` is the entry point every slide loads. Hot reload picks up a
change immediately, reloading only the frames that use the tag.

## The rules

**Use a project prefix.** `deck-*` is reserved for the built-ins. A Custom
Element name must contain a hyphen — a platform rule, not a deck one.

**Prefer Light DOM.** Render into the normal document and everything keeps
working: ordinary CSS selectors, Tailwind utilities, Anime.js, the layout checks
and DevTools. Reach for Shadow DOM only when hiding internal structure genuinely
pays for itself — a chart building its own SVG, a widget wrapping a third-party
library. Among the built-ins, only `deck-code` qualifies.

**Keep generated markup idempotent.** `connectedCallback` can run more than once,
so check before you create.

**Style it from `design/`, not from JavaScript.** Light DOM means a plain
selector works, and the styles then live with the rest of the deck's look.

```js
class AcmeMetric extends HTMLElement {
  static observedAttributes = ["label"];

  connectedCallback() {
    this.#render();
  }

  attributeChangedCallback() {
    if (this.isConnected) {
      this.#render();
    }
  }

  #render() {
    let label = this.querySelector(":scope > .acme-metric__label");
    const text = this.getAttribute("label");
    if (!text) {
      label?.remove();
      return;
    }
    if (!label) {
      label = document.createElement("span");
      label.className = "acme-metric__label";
      this.prepend(label);
    }
    label.textContent = text;
  }
}

customElements.define("acme-metric", AcmeMetric);
```

```css
/* design/theme.css */
acme-metric {
  display: flex;
  gap: var(--deck-space-2);
  color: var(--deck-color-muted);
}
```

## Animation: on reveal, never on construction

This is the rule that catches everyone. An element is constructed when its
**iframe** is created, and the shell preloads the neighbouring slides — so an
animation started from `connectedCallback` runs while the slide is off-screen,
finishes before you arrive, and then appears to fire at random depending on how
far away the previous slide was.

`deck.onReveal` ties it to visibility instead:

```js
import { animate } from "/@deck/vendor/animejs.js";

window.deck.onReveal(element, ({ signal, step }) => {
  const animation = animate(element, { opacity: [0, 1], translateY: [12, 0] });
  signal.addEventListener("abort", () => animation.revert());
});
```

It fires when the slide is entered **and** the element's `data-step` threshold is
reached, and the `AbortSignal` aborts as soon as either stops being true — so the
animation cannot run off-screen, cannot outlive its reveal, and replays when you
step back and forward again.

`deck.animator()` returns `null` when animation is off (reduced motion, print and
check modes), so handle both paths:

```js
const animate = await window.deck.animator();
if (!animate) {
  element.textContent = finalValue;   // final state, no animation
  return;
}
```

For a long timeline, register it so printing seeks it to the end:

```js
window.deck.registerTimeline(createTimeline());
```

## Reading the deck's state

`window.deck` is available in every slide, so a component can react to the deck
rather than only to its own attributes:

| | |
|---|---|
| `mode` | `present`, `presenter`, `print`, `check`, `standalone` |
| `slideId` `step` `stepCount` | where you are |
| `position` `whenPositioned()` | `{ index, number, total }` from the manifest |
| `canvas` | `{ width, height }` |
| `onReveal(element, cb)` | run `cb` when the element becomes visible |
| `animator()` `anime()` | Anime.js, or `null` when animation is off |
| `registerTimeline(t)` | seek `t` to the end before printing |
| `waitUntil(promise)` | delay `deck:ready` until it settles |
| `setStepCount(n)` | declare steps not expressed with `data-step` |

```js
window.deck.whenPositioned().then(({ number, total }) => {
  this.textContent = `${number} / ${total}`;
});
```

If a component fetches or measures something before the slide is presentable,
hold readiness so printing and checking wait for it:

```js
document.addEventListener("deck:init", (event) => {
  event.detail.waitUntil(loadChartData());
});
```

## Checking

`deck check` knows which tags exist. A typo, or a component you forgot to
import, is reported as `invalid_component_name` or `undefined_component` rather
than silently rendering nothing — an undefined Custom Element is an empty inline
box, easy to miss on a slide.

```bash
deck check --static      # names and imports, no browser
deck check               # plus anything the component throws at runtime
```
