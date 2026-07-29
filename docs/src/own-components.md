# Adding your own components

A component is a Custom Element. There is no registry to declare, no build step and no
framework — if `customElements.define()` runs, the element works in every slide.

## Scaffold one

```bash
deck component new acme-metric
```

That writes `components/acme-metric.js` and adds the import to `components/index.js`,
which is the entry point every slide loads. Hot reload picks it up immediately: only the
frames that actually use the tag are reloaded.

## The rules

**Use a project prefix.** `deck-*` is reserved for the built-ins. `acme-*`, `rccs-*`,
anything of yours — and a Custom Element name must contain a hyphen, which is a platform
rule rather than a deck one.

**Prefer Light DOM.** Render into the normal document and everything keeps working:
ordinary CSS selectors, Tailwind utilities, Anime.js, the layout checks, and DevTools.
Reach for Shadow DOM only when hiding the internal structure genuinely pays for itself —
a chart that builds its own SVG, a widget wrapping a third-party library. In the built-in
set, only `deck-code` qualifies.

**Keep generated markup idempotent.** `connectedCallback` can run more than once, so
check before you create:

```js
class AcmeMetric extends HTMLElement {
  connectedCallback() {
    let label = this.querySelector(":scope > .acme-metric__label");
    if (!label) {
      label = document.createElement("span");
      label.className = "acme-metric__label";
      this.prepend(label);
    }
    label.textContent = this.getAttribute("label") ?? "";
  }
}

customElements.define("acme-metric", AcmeMetric);
```

**Style it from `design/`, not from JavaScript.** Light DOM means a plain selector works,
and the styles then live with the rest of the deck's look:

```css
/* design/theme.css */
acme-metric {
  display: flex;
  gap: var(--deck-space-2);
  color: var(--deck-color-muted);
}
```

**Never animate from `connectedCallback`.** An element is constructed when its iframe is
created, and the shell preloads the neighbouring slides — so the animation would run
off-screen and appear to fire at random. Use `deck.onReveal`, covered in
[Animation](./animation.md):

```js
window.deck.onReveal(this, ({ signal }) => {
  const animation = animate(this, { opacity: [0, 1] });
  signal.addEventListener("abort", () => animation.revert());
});
```

## Reading the deck's state

`window.deck` is available inside every slide, so a component can react to the deck
rather than just to its own attributes:

```js
window.deck.whenPositioned().then(({ number, total }) => {
  this.textContent = `${number} / ${total}`;
});
```

See [Animation](./animation.md#the-runtime-api) for the full surface.

## Checking

`deck check` knows which tags exist. A typo in a tag name, or a component you forgot to
import, is reported as `invalid_component_name` rather than silently rendering nothing —
an undefined Custom Element is an empty inline box, which is easy to miss on a slide.

```bash
deck component list      # built-ins, plus every tag found under components/
```
