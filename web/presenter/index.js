// Deck index page (/).
import { env } from "./env.js";

const base = new URL("../", import.meta.url).pathname;

const title = document.querySelector("[data-deck-title]");
const meta = document.querySelector("[data-deck-meta]");
const list = document.querySelector("[data-deck-list]");

title.textContent = env.deck.title;
document.title = env.deck.title;
document.documentElement.lang = env.deck.lang;

for (const link of document.querySelectorAll("[data-deck-link]")) {
  link.href = `${base}${link.dataset.deckLink}`;
}

const response = await fetch(`${base}@deck/manifest.json`, { cache: "no-store" });
const manifest = await response.json();

meta.textContent = [
  env.deck.author && `${env.deck.author}`,
  `${manifest.slides.length} slide${manifest.slides.length === 1 ? "" : "s"}`,
  `canvas ${env.canvas.width}×${env.canvas.height}`,
]
  .filter(Boolean)
  .join(" · ");

for (const slide of manifest.slides) {
  const item = document.createElement("li");
  const link = document.createElement("a");
  link.href = `${base}slides/${slide.id}`;

  const order = document.createElement("span");
  order.className = "deck-index__order";
  order.textContent = String(slide.order + 1).padStart(2, "0");

  const name = document.createElement("strong");
  name.textContent = slide.title || slide.id;

  const path = document.createElement("span");
  path.className = "deck-index__path";
  path.textContent = `slides/${slide.path}`;

  link.append(order, name, path);
  item.append(link);
  list.append(item);
}
