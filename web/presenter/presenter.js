// Presenter view (/presenter): current slide, next slide, notes, timer, controls.
import { DeckShell, installKeyboard } from "./shell.js";
import { env } from "./env.js";

const base = new URL("../", import.meta.url).pathname;

const layer = document.querySelector("[data-deck-layer]");
const regions = {
  current: document.querySelector('[data-deck-region="current"]'),
  next: document.querySelector('[data-deck-region="next"]'),
};

const ui = {
  position: document.querySelector("[data-deck-position]"),
  slideTitle: document.querySelector("[data-deck-slide-title]"),
  nextTitle: document.querySelector("[data-deck-next-title]"),
  notes: document.querySelector("[data-deck-notes]"),
  diagnostics: document.querySelector("[data-deck-diagnostics]"),
  timer: document.querySelector("[data-deck-timer]"),
  timerToggle: document.querySelector("[data-deck-timer-toggle]"),
  timerReset: document.querySelector("[data-deck-timer-reset]"),
  now: document.querySelector("[data-deck-now]"),
  connection: document.querySelector("[data-deck-connection]"),
  blackout: document.querySelector("[data-deck-blackout]"),
  pause: document.querySelector("[data-deck-pause]"),
  printLink: document.querySelector("[data-deck-print-link]"),
  presentLink: document.querySelector("[data-deck-present-link]"),
};

ui.printLink.href = `${base}print`;
ui.presentLink.href = `${base}present`;

/* -- timer ------------------------------------------------------------- */

const TIMER_KEY = "deck.presenter.timer";

const timer = loadTimer();

function loadTimer() {
  try {
    return JSON.parse(sessionStorage.getItem(TIMER_KEY)) ?? { running: false, startedAt: 0, accumulated: 0 };
  } catch {
    return { running: false, startedAt: 0, accumulated: 0 };
  }
}

function saveTimer() {
  try {
    sessionStorage.setItem(TIMER_KEY, JSON.stringify(timer));
  } catch {
    /* private mode */
  }
}

function elapsedMs() {
  return timer.accumulated + (timer.running ? Date.now() - timer.startedAt : 0);
}

function formatDuration(ms) {
  const total = Math.max(Math.floor(ms / 1000), 0);
  const hours = String(Math.floor(total / 3600)).padStart(2, "0");
  const minutes = String(Math.floor((total % 3600) / 60)).padStart(2, "0");
  const seconds = String(total % 60).padStart(2, "0");
  return `${hours}:${minutes}:${seconds}`;
}

function toggleTimer() {
  if (timer.running) {
    timer.accumulated = elapsedMs();
    timer.running = false;
  } else {
    timer.startedAt = Date.now();
    timer.running = true;
  }
  saveTimer();
  renderTimer();
  shell.publish();
}

function resetTimer() {
  timer.running = false;
  timer.accumulated = 0;
  timer.startedAt = 0;
  saveTimer();
  renderTimer();
  shell.publish();
}

function renderTimer() {
  ui.timer.textContent = formatDuration(elapsedMs());
  ui.timer.classList.toggle("is-running", timer.running);
  ui.timerToggle.textContent = timer.running ? "Pause" : "Start";
}

/* -- shell ------------------------------------------------------------- */

const shell = new DeckShell({
  base,
  mode: "presenter",
  layer,
  preload: Math.max(env.server.preload, 1),
  canvas: env.canvas,
  regionForOffset: (offset) => (offset === 0 ? "current" : offset === 1 ? "next" : null),
  regionRect: (name) => regions[name]?.getBoundingClientRect() ?? null,
});

shell.extraSyncState = () => ({ timer: { ...timer } });

shell.on("change", (snapshot) => {
  if (!snapshot.slide) {
    ui.position.textContent = "No slides";
    return;
  }
  const steps = snapshot.stepCount > 0 ? `  step ${snapshot.step}/${snapshot.stepCount}` : "";
  ui.position.textContent = `${snapshot.index + 1} / ${snapshot.total}${steps}`;
  ui.slideTitle.textContent = snapshot.slide.title ?? snapshot.slide.id;

  const nextSlide = shell.slides[snapshot.index + 1];
  ui.nextTitle.textContent = nextSlide ? (nextSlide.title ?? nextSlide.id) : "— end of deck —";

  const notes = snapshot.frame?.meta?.notes || snapshot.slide.notes || "";
  ui.notes.innerHTML = notes || '<p class="presenter-notes__empty">No speaker notes</p>';

  ui.blackout.classList.toggle("is-active", snapshot.blackout);
  renderDiagnostics(snapshot.frame);
});

function renderDiagnostics(frame) {
  const items = frame?.diagnostics ?? [];
  ui.diagnostics.innerHTML = "";
  if (items.length === 0) {
    ui.diagnostics.innerHTML = '<li class="is-ok">No problems detected</li>';
    return;
  }
  for (const item of items) {
    const li = document.createElement("li");
    li.className = `is-${item.severity ?? "error"}`;
    li.textContent = `[${item.rule}] ${item.message}`;
    ui.diagnostics.append(li);
  }
}

shell.on("connection", ({ connected }) => {
  ui.connection.textContent = connected ? "connected" : "offline";
  ui.connection.classList.toggle("is-offline", !connected);
});

shell.on("remote-state", (state) => {
  if (state.timer && typeof state.timer.accumulated === "number") {
    Object.assign(timer, state.timer);
    saveTimer();
    renderTimer();
  }
});

ui.timerToggle.addEventListener("click", toggleTimer);
ui.timerReset.addEventListener("click", resetTimer);
ui.blackout.addEventListener("click", () => shell.toggleBlackout());
ui.pause.addEventListener("click", () => {
  shell.paused = !shell.paused;
  ui.pause.classList.toggle("is-active", shell.paused);
  const frame = shell.currentFrame();
  frame?.post(shell.paused ? "pause" : "resume");
  shell.publish();
});

installKeyboard(shell, {
  t: toggleTimer,
  r: resetTimer,
});

setInterval(() => {
  renderTimer();
  ui.now.textContent = new Date().toLocaleTimeString("ja-JP", { hour12: false });
}, 250);

renderTimer();
// Exposed for debugging and for the end-to-end tests.
window.deckShell = shell;

await shell.start();
