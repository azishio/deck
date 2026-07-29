// A tiny project component: three stacked cards that fan out on `deck:enter`.
// It exists to demonstrate that a user-defined Custom Element, Light DOM CSS
// and Anime.js all work together inside a slide.
class DeckLogo extends HTMLElement {
  connectedCallback() {
    if (this.querySelector(":scope > .deck-logo__card")) {
      return;
    }
    for (let index = 0; index < 3; index += 1) {
      const card = document.createElement("span");
      card.className = "deck-logo__card";
      card.dataset.index = String(index);
      this.append(card);
    }
    this.#animate();
  }

  async #animate() {
    const cards = [...this.querySelectorAll(":scope > .deck-logo__card")];
    const animate = await (window.deck?.animator?.() ?? Promise.resolve(null));
    if (!animate) {
      return;
    }
    animate(cards, {
      rotate: [0, (_, index) => (index - 1) * 8],
      translateX: [0, (_, index) => (index - 1) * 18],
      duration: 900,
      ease: "outExpo",
      delay: (_, index) => index * 90,
    });
  }
}

customElements.define("deck-logo", DeckLogo);
