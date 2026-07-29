// Print view (/print). Loads every slide in print mode, drives each one to the
// requested step, waits for fonts/images/slide-owned promises, then enables the
// print button. `window.print()` is never called automatically.
import { env } from "./env.js";

const PROTOCOL = Object.freeze({ namespace: "deck", version: 1 });
const CONCURRENCY = 4;

const base = new URL("../", import.meta.url).pathname;
const params = new URLSearchParams(location.search);
const steps = params.get("steps") ?? env.print.steps;
const slideFilter = params.get("slide");

const ui = {
  status: document.querySelector("[data-deck-status]"),
  stepsSelect: document.querySelector("[data-deck-steps]"),
  printButton: document.querySelector("[data-deck-print]"),
  pages: document.querySelector("[data-deck-pages]"),
  diagnostics: document.querySelector("[data-deck-diagnostics]"),
  diagnosticsList: document.querySelector("[data-deck-diagnostics-list]"),
};

ui.stepsSelect.value = steps;
ui.stepsSelect.addEventListener("change", () => {
  const url = new URL(location.href);
  url.searchParams.set("steps", ui.stepsSelect.value);
  location.href = url.toString();
});
ui.printButton.addEventListener("click", () => window.print());

document.documentElement.style.setProperty("--deck-canvas-width", `${env.canvas.width}px`);
document.documentElement.style.setProperty("--deck-canvas-height", `${env.canvas.height}px`);

// @page cannot read custom properties, so derive the sheet size (CSS px -> in).
const pageStyle = document.createElement("style");
pageStyle.textContent = `@page { size: ${env.canvas.width / 96}in ${env.canvas.height / 96}in; margin: 0; }`;
document.head.append(pageStyle);

const response = await fetch(`${base}@deck/manifest.json`, { cache: "no-store" });
const manifest = await response.json();
document.title = `${env.deck.title} — print`;

const slides = manifest.slides.filter((slide) => !slideFilter || slide.id === slideFilter);

/** Expand the manifest into printable pages according to `steps`. */
function buildPages() {
  const pages = [];
  for (const slide of slides) {
    const stepCount = slide.stepCount ?? 0;
    if (steps === "each" && stepCount > 0) {
      for (let step = 0; step <= stepCount; step += 1) {
        pages.push({ slide, step });
      }
    } else if (steps === "initial") {
      pages.push({ slide, step: 0 });
    } else {
      pages.push({ slide, step: "final" });
    }
  }
  return pages;
}

const pages = buildPages();
const pending = new Map();
const diagnostics = [];
let readyCount = 0;

function slideUrl(slide, step) {
  const url = new URL(`${base}slides/${slide.id}`.replace(/\/{2,}/g, "/"), location.href);
  url.searchParams.set("deck-mode", "print");
  url.searchParams.set("step", String(step));
  return url.toString();
}

function renderStatus() {
  ui.status.textContent = `${readyCount} / ${pages.length} slides ready`;
  if (readyCount === pages.length) {
    ui.status.textContent += diagnostics.length > 0 ? ` · ${diagnostics.length} 件の指摘` : " · OK";
    ui.printButton.disabled = false;
    document.documentElement.dataset.deckPrintReady = "true";
  }
}

function renderDiagnostics() {
  if (!env.print.preflight) {
    return;
  }
  ui.diagnostics.hidden = false;
  ui.diagnosticsList.innerHTML = "";

  const summary = [
    { ok: readyCount === pages.length, text: `${readyCount} / ${pages.length} slides loaded` },
  ];
  for (const line of summary) {
    const li = document.createElement("li");
    li.className = line.ok ? "is-ok" : "is-pending";
    li.textContent = `${line.ok ? "✓" : "…"} ${line.text}`;
    ui.diagnosticsList.append(li);
  }
  for (const item of diagnostics) {
    const li = document.createElement("li");
    const severity = item.severity ?? "error";
    li.className = `is-${severity}`;
    li.textContent = `${severity === "error" ? "✗" : "⚠"} ${item.slideId}: [${item.rule}] ${item.message}`;
    ui.diagnosticsList.append(li);
  }
}

window.addEventListener("message", (event) => {
  if (event.origin !== location.origin) {
    return;
  }
  const message = event.data;
  if (!message || message.namespace !== PROTOCOL.namespace || message.version !== PROTOCOL.version) {
    return;
  }
  const entry = [...pending.values()].find((item) => item.iframe.contentWindow === event.source);
  if (!entry) {
    return;
  }
  const payload = message.payload ?? {};

  if (message.type === "ready") {
    entry.iframe.contentWindow.postMessage(
      {
        ...PROTOCOL,
        type: "prepare-print",
        slideId: entry.page.slide.id,
        payload: { step: entry.page.step },
      },
      location.origin,
    );
    if (env.print.showNotes && payload.notes) {
      entry.notes.innerHTML = payload.notes;
      entry.notes.hidden = false;
    }
  } else if (message.type === "print-ready") {
    for (const item of payload.diagnostics ?? []) {
      diagnostics.push({ ...item, slideId: entry.page.slide.id });
    }
    readyCount += 1;
    pending.delete(entry.index);
    entry.resolve();
    renderStatus();
    renderDiagnostics();
  } else if (message.type === "diagnostic") {
    diagnostics.push({ ...payload, slideId: entry.page.slide.id });
  }
});

function createPage(page, index) {
  const section = document.createElement("section");
  section.className = "print-page";
  section.dataset.slideId = page.slide.id;
  section.dataset.step = String(page.step);

  const iframe = document.createElement("iframe");
  iframe.title = `${page.slide.title ?? page.slide.id} (step ${page.step})`;
  iframe.src = slideUrl(page.slide, page.step);
  section.append(iframe);

  const notes = document.createElement("aside");
  notes.className = "print-notes";
  notes.hidden = true;
  section.append(notes);

  ui.pages.append(section);

  return new Promise((resolve) => {
    pending.set(index, { index, page, iframe, notes, resolve });
    setTimeout(() => {
      if (pending.delete(index)) {
        diagnostics.push({
          slideId: page.slide.id,
          rule: "ready-timeout",
          severity: "error",
          message: `${env.readyTimeoutMs}ms 以内に print-ready になりませんでした`,
        });
        readyCount += 1;
        renderStatus();
        renderDiagnostics();
        resolve();
      }
    }, env.readyTimeoutMs);
  });
}

renderStatus();
renderDiagnostics();

// Load in batches: printing needs every frame in the DOM, but loading a hundred
// iframes at once starves the renderer.
for (let start = 0; start < pages.length; start += CONCURRENCY) {
  const batch = pages.slice(start, start + CONCURRENCY);
  await Promise.all(batch.map((page, offset) => createPage(page, start + offset)));
}
