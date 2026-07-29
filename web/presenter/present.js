// Audience presentation view (/present).
import { DeckShell, installKeyboard } from "./shell.js";
import { env } from "./env.js";

const base = new URL("../", import.meta.url).pathname;

const layer = document.querySelector("[data-deck-layer]");
const stage = document.querySelector('[data-deck-region="current"]');
const blackout = document.querySelector(".deck-blackout");
const position = document.querySelector("[data-deck-position]");
const titleLabel = document.querySelector("[data-deck-title]");
const connection = document.querySelector("[data-deck-connection]");
const progress = document.querySelector("[data-deck-progress]");
const toast = document.querySelector("[data-deck-toast]");

const shell = new DeckShell({
  base,
  mode: "present",
  layer,
  preload: env.server.preload,
  canvas: env.canvas,
  regionForOffset: (offset) => (offset === 0 ? "current" : null),
  regionRect: (name) => (name === "current" ? stage.getBoundingClientRect() : null),
});

// Match the deck's custom cursor over the shell chrome and the letterbox.
if (env.cursor) {
  document.documentElement.style.cursor = `url("${base}${env.cursor}") 0 0, auto`;
}

let toastTimer = 0;
function showToast(message) {
  toast.textContent = message;
  toast.hidden = false;
  clearTimeout(toastTimer);
  toastTimer = setTimeout(() => {
    toast.hidden = true;
  }, 1800);
}

shell.on("change", (snapshot) => {
  if (!snapshot.slide) {
    position.textContent = "No slides";
    return;
  }
  const steps = snapshot.stepCount > 0 ? ` · ${snapshot.step}/${snapshot.stepCount}` : "";
  position.textContent = `${snapshot.index + 1} / ${snapshot.total}${steps}`;
  titleLabel.textContent = snapshot.slide.title ?? snapshot.slide.id;
  document.title = `${snapshot.slide.title ?? snapshot.slide.id} — ${env.deck.title}`;
  progress.style.width = `${((snapshot.index + 1) / Math.max(snapshot.total, 1)) * 100}%`;
  blackout.hidden = !snapshot.blackout;
});

shell.on("connection", ({ connected }) => {
  connection.textContent = connected ? "" : "offline";
  connection.classList.toggle("is-offline", !connected);
});

shell.on("hot", (message) => {
  if (message.type === "slide-changed") {
    showToast("Slide reloaded");
  } else if (message.type === "style-changed") {
    showToast("Styles updated");
  } else if (message.type === "manifest-changed") {
    showToast("Slide list updated");
  }
});

installKeyboard(shell, {
  p: () => window.open(`${base}presenter${location.hash}`, "deck-presenter"),
  s: () => window.open(`${base}presenter${location.hash}`, "deck-presenter"),
});

// Clicks on the slide itself are forwarded by its runtime; this covers the
// letterbox around it, with the same left/right split.
document.addEventListener("click", (event) => {
  if (event.target.closest(".deck-hud")) {
    return;
  }
  if (event.clientX >= innerWidth / 2) {
    shell.next();
  } else {
    shell.previous();
  }
});

document.addEventListener("contextmenu", (event) => {
  event.preventDefault();
  shell.previous();
});

let idleTimer = 0;
function markActive() {
  document.body.classList.remove("is-idle");
  clearTimeout(idleTimer);
  idleTimer = setTimeout(() => document.body.classList.add("is-idle"), 2500);
}
document.addEventListener("mousemove", markActive);
markActive();

// Exposed for debugging and for the end-to-end tests.
window.deckShell = shell;

await shell.start();
