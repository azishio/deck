// A tiny project component: three stacked cards that fan out when the slide is
// shown. It exists to demonstrate that a user-defined Custom Element, Light DOM
// CSS and Anime.js all work together inside a slide.
//
// The animation is driven by `deck.onReveal` rather than `connectedCallback`,
// because an element is constructed when its iframe is created — which the
// presentation shell may do while the slide is still an off-screen preload.
class DeckLogo extends HTMLElement {
  #stopWatching = null;

  connectedCallback() {
    if (!this.querySelector(":scope > .deck-logo__card")) {
      for (let index = 0; index < 3; index += 1) {
        const card = document.createElement("span");
        card.className = "deck-logo__card";
        card.dataset.index = String(index);
        this.append(card);
      }
    }
    this.#stopWatching = window.deck?.onReveal?.(this, (reveal) => this.#fanOut(reveal));
  }

  disconnectedCallback() {
    this.#stopWatching?.();
    this.#stopWatching = null;
  }

  async #fanOut({ signal }) {
    const cards = [...this.querySelectorAll(":scope > .deck-logo__card")];
    const animate = await (window.deck?.animator?.() ?? Promise.resolve(null));
    if (!animate || signal.aborted) {
      return;
    }

    const fanned = animate(cards, {
      rotate: [0, (_, index) => (index - 1) * 8],
      translateX: [0, (_, index) => (index - 1) * 18],
      duration: 900,
      ease: "outExpo",
      delay: (_, index) => index * 90,
    });

    // Stepping away mid-flight must not leave the cards halfway.
    signal.addEventListener("abort", () => fanned?.revert?.());
  }
}

customElements.define("deck-logo", DeckLogo);
