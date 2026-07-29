// Slide runtime. Runs inside every slide iframe.
//
// Responsibilities:
//   - expose `window.deck`
//   - dispatch the `deck:*` DOM events
//   - own the absolute step model (screen state = f(slide_id, step))
//   - implement the readiness protocol (design doc 12)
//   - speak the parent <-> iframe postMessage protocol (design doc 8.4)

import { env } from "./env.js";

export const PROTOCOL = Object.freeze({ namespace: "deck", version: 1 });

/** Base URL of the deck ("/" during dev, `base_url` for a static build). */
const DECK_BASE = new URL("../", import.meta.url).pathname;

const params = new URLSearchParams(location.search);
const embedded = window.parent !== window;

const state = {
  mode: params.get("deck-mode") ?? (embedded ? "present" : "standalone"),
  slideId: null,
  step: 0,
  stepCount: 0,
  reducedMotion: false,
  ready: false,
  visible: true,
  position: { index: 0, number: 0, total: 0 },
};

const readyPromises = [];
const timelines = [];
const diagnostics = [];
let usedTags = [];
let animePromise = null;
let positionPromise = null;

/* -------------------------------------------------------------------------- */
/* utilities                                                                   */
/* -------------------------------------------------------------------------- */

// requestAnimationFrame does not fire while a document is not being rendered
// (hidden tab, preloaded frame). Readiness must still settle, so fall back to a
// timer instead of stalling forever.
const nextFrame = (fallbackMs = 50) =>
  new Promise((resolve) => {
    let done = false;
    const finish = () => {
      if (!done) {
        done = true;
        resolve();
      }
    };
    requestAnimationFrame(finish);
    setTimeout(finish, fallbackMs);
  });

const domContentLoaded = () =>
  document.readyState === "loading"
    ? new Promise((resolve) =>
        document.addEventListener("DOMContentLoaded", () => resolve(), { once: true }),
      )
    : Promise.resolve();

function withTimeout(promise, ms, onTimeout) {
  let timer;
  return Promise.race([
    promise.finally(() => clearTimeout(timer)),
    new Promise((resolve) => {
      timer = setTimeout(() => resolve(onTimeout?.()), ms);
    }),
  ]);
}

function dispatch(type, detail = {}) {
  const event = new CustomEvent(type, { detail, bubbles: true, cancelable: false });
  document.dispatchEvent(event);
  return event;
}

function post(type, payload = {}) {
  if (!embedded) {
    return;
  }
  window.parent.postMessage(
    { ...PROTOCOL, type, slideId: state.slideId, payload },
    location.origin,
  );
}

function addDiagnostic(rule, severity, message, extra = {}) {
  diagnostics.push({ rule, severity, message, ...extra });
  post("diagnostic", { rule, severity, message, ...extra });
}

function resolveReducedMotion() {
  if (params.get("deck-reduced-motion") === "true") {
    return true;
  }
  if (params.get("deck-reduced-motion") === "false") {
    return false;
  }
  if (env.animation.reducedMotion === "ignore") {
    return false;
  }
  return matchMedia("(prefers-reduced-motion: reduce)").matches;
}

/* -------------------------------------------------------------------------- */
/* anime.js access                                                             */
/* -------------------------------------------------------------------------- */

function loadAnime() {
  if (env.animation.engine !== "animejs") {
    return Promise.resolve(null);
  }
  animePromise ??= import(`${DECK_BASE}@deck/vendor/animejs.js`).catch((error) => {
    addDiagnostic("animation-engine", "warning", `anime.js を読み込めませんでした: ${error}`);
    return null;
  });
  return animePromise;
}

/**
 * Where this slide sits in the deck.
 *
 * A slide document has no idea of its own order — that lives in the manifest —
 * so components such as `deck-slide-number` resolve it here once per document.
 */
function loadPosition() {
  positionPromise ??= fetch(`${DECK_BASE}@deck/manifest.json`, { cache: "no-store" })
    .then((response) => response.json())
    .then((manifest) => {
      const index = manifest.slides.findIndex((slide) => slide.id === state.slideId);
      return {
        index: Math.max(index, 0),
        number: Math.max(index, 0) + 1,
        total: manifest.slides.length,
      };
    })
    .catch(() => ({ index: 0, number: 0, total: 0 }));
  return positionPromise;
}

/* -------------------------------------------------------------------------- */
/* step engine                                                                 */
/* -------------------------------------------------------------------------- */

function stepElements() {
  return [...document.querySelectorAll("[data-step]")]
    .map((element) => ({ element, step: Number.parseInt(element.dataset.step ?? "0", 10) || 0 }))
    .filter((entry) => entry.step > 0);
}

let declaredStepCount = null;

function computeStepCount() {
  const fromDom = stepElements().reduce((max, entry) => Math.max(max, entry.step), 0);
  return Math.max(fromDom, declaredStepCount ?? 0);
}

function clampStep(step) {
  if (step === "final") {
    return state.stepCount;
  }
  if (step === "initial" || step === null || step === undefined || step === "") {
    return 0;
  }
  const value = Number.parseInt(step, 10);
  if (!Number.isFinite(value)) {
    return 0;
  }
  return Math.min(Math.max(value, 0), state.stepCount);
}

async function animateAppearing(elements) {
  const anime = await loadAnime();
  if (!anime) {
    for (const element of elements) {
      element.style.removeProperty("opacity");
      element.style.removeProperty("transform");
    }
    return;
  }
  const styles = getComputedStyle(document.documentElement);
  const duration = Number.parseFloat(styles.getPropertyValue("--deck-step-duration")) || 420;
  const distance = Number.parseFloat(styles.getPropertyValue("--deck-step-distance")) || 12;
  anime.animate(elements, {
    opacity: [0, 1],
    translateY: [distance, 0],
    duration,
    ease: "outQuad",
    delay: anime.stagger(60),
    onComplete: () => {
      for (const element of elements) {
        element.style.removeProperty("opacity");
        element.style.removeProperty("transform");
      }
    },
  });
}

function applyStep(step, { instant = false, direction = "forward", silent = false } = {}) {
  const from = state.step;
  const to = clampStep(step);
  state.step = to;

  const appearing = [];
  for (const { element, step: threshold } of stepElements()) {
    const active = to >= threshold;
    element.dataset.deckStepState = active ? "active" : "pending";
    if (!active) {
      element.style.removeProperty("opacity");
      element.style.removeProperty("transform");
    } else if (threshold > from && !instant) {
      appearing.push(element);
    } else {
      element.style.removeProperty("opacity");
      element.style.removeProperty("transform");
    }
  }

  const useAnimation = !instant && !state.reducedMotion && appearing.length > 0;
  if (useAnimation) {
    void animateAppearing(appearing);
  }

  if (!silent) {
    dispatch("deck:stepchange", {
      slideId: state.slideId,
      from,
      to,
      direction,
      instant: instant || state.reducedMotion,
    });
    post("step-changed", { step: to, from, stepCount: state.stepCount });
  }
  return to;
}

/* -------------------------------------------------------------------------- */
/* readiness                                                                   */
/* -------------------------------------------------------------------------- */

function collectCustomElementTags() {
  return [
    ...new Set(
      [...document.querySelectorAll("*")]
        .map((element) => element.localName)
        .filter((name) => name.includes("-")),
    ),
  ];
}

async function waitForCustomElements(tags) {
  const pending = tags.filter((tag) => !customElements.get(tag));
  if (pending.length === 0) {
    return;
  }
  await withTimeout(
    Promise.all(pending.map((tag) => customElements.whenDefined(tag))),
    env.readyTimeoutMs,
    () => {
      for (const tag of pending.filter((tag) => !customElements.get(tag))) {
        addDiagnostic(
          "undefined-component",
          "error",
          `Custom Element <${tag}> が定義されませんでした`,
          { selector: tag },
        );
      }
    },
  );
}

// Tailwind's browser build compiles asynchronously after DOMContentLoaded.
// Its generated stylesheet always starts with the layer declaration, so an
// empty one means the first compilation has not finished yet.
async function waitForTailwind() {
  const compiled = () =>
    [...document.querySelectorAll("style:not([type])")].some((style) =>
      style.textContent.includes("@layer theme"),
    );

  const deadline = performance.now() + env.tailwindTimeoutMs;
  while (!compiled()) {
    if (performance.now() > deadline) {
      addDiagnostic(
        "tailwind-timeout",
        "warning",
        `Tailwind CSS が ${env.tailwindTimeoutMs}ms 以内にコンパイルされませんでした`,
      );
      return;
    }
    await nextFrame();
  }
}

async function decodeImages() {
  const images = [...document.images];
  await Promise.allSettled(
    images.map(async (image) => {
      try {
        await image.decode();
      } catch {
        addDiagnostic("missing-asset", "error", `画像を読み込めませんでした: ${image.currentSrc || image.src}`, {
          selector: cssPath(image),
          url: image.currentSrc || image.src,
        });
      }
    }),
  );
}

function cssPath(element) {
  if (!element || element === document.documentElement) {
    return "html";
  }
  if (element.id) {
    return `#${CSS.escape(element.id)}`;
  }
  const parent = element.parentElement;
  if (!parent) {
    return element.localName;
  }
  const siblings = [...parent.children].filter((child) => child.localName === element.localName);
  const nth = siblings.length > 1 ? `:nth-of-type(${siblings.indexOf(element) + 1})` : "";
  return `${cssPath(parent)} > ${element.localName}${nth}`;
}

function quickLayoutCheck() {
  const root = document.querySelector("deck-slide") ?? document.body;
  const overflowX = root.scrollWidth - env.canvas.width;
  const overflowY = root.scrollHeight - env.canvas.height;
  const tolerance = env.check.overflowTolerancePx;
  if (overflowX > tolerance || overflowY > tolerance) {
    addDiagnostic(
      "slide-overflow",
      "error",
      `スライドがcanvasを超えています (+${Math.max(overflowX, 0)}px, +${Math.max(overflowY, 0)}px)`,
      { selector: cssPath(root) },
    );
  }
  return { overflowX, overflowY };
}

function resourceUrls() {
  try {
    return performance.getEntriesByType("resource").map((entry) => entry.name);
  } catch {
    return [];
  }
}

function notesHtml() {
  return [...document.querySelectorAll("deck-notes")].map((node) => node.innerHTML.trim()).join("\n");
}

async function becomeReady() {
  await domContentLoaded();

  state.slideId =
    document.querySelector("deck-slide")?.id ||
    params.get("deck-slide-id") ||
    document.title ||
    "slide";

  state.reducedMotion = resolveReducedMotion();
  document.documentElement.dataset.deckReducedMotion = String(state.reducedMotion);
  document.documentElement.dataset.deckMode = state.mode;

  state.stepCount = computeStepCount();
  applyStep(params.get("step"), { instant: true, silent: true });

  dispatch("deck:init", {
    slideId: state.slideId,
    mode: state.mode,
    step: state.step,
    stepCount: state.stepCount,
    reducedMotion: state.reducedMotion,
    waitUntil: (promise) => readyPromises.push(Promise.resolve(promise)),
  });

  usedTags = collectCustomElementTags();
  state.position = await loadPosition();
  await waitForCustomElements(usedTags);

  // Custom Elements may add [data-step] children of their own.
  state.stepCount = computeStepCount();
  applyStep(state.step, { instant: true, silent: true });

  await waitForTailwind();
  await document.fonts.ready;
  await decodeImages();
  await Promise.allSettled(readyPromises);
  await nextFrame();
  await nextFrame();

  const layout = quickLayoutCheck();
  state.ready = true;

  dispatch("deck:ready", { slideId: state.slideId, step: state.step, stepCount: state.stepCount });
  post("ready", {
    slideId: state.slideId,
    title: document.title,
    position: state.position,
    step: state.step,
    stepCount: state.stepCount,
    notes: notesHtml(),
    tags: usedTags,
    resources: resourceUrls(),
    diagnostics,
    layout,
  });

  document.documentElement.dataset.deckReady = "true";
}

/* -------------------------------------------------------------------------- */
/* print                                                                       */
/* -------------------------------------------------------------------------- */

async function preparePrint(payload) {
  const requested = payload?.step ?? "final";
  applyStep(requested, { instant: true, silent: true });

  await waitForTailwind();
  const promises = [];
  dispatch("deck:prepare-print", {
    slideId: state.slideId,
    step: requested,
    resolvedStep: state.step,
    waitUntil: (promise) => promises.push(Promise.resolve(promise)),
  });

  for (const timeline of timelines) {
    try {
      timeline.seek(timeline.duration);
    } catch {
      /* a slide-owned timeline may already be disposed */
    }
  }

  await document.fonts.ready;
  await decodeImages();
  await Promise.allSettled(promises);
  await nextFrame();
  await nextFrame();

  const layout = quickLayoutCheck();
  post("print-ready", { slideId: state.slideId, step: state.step, diagnostics, layout });
}

/* -------------------------------------------------------------------------- */
/* stylesheet hot replacement (design doc 11.5)                                */
/* -------------------------------------------------------------------------- */

async function replaceStylesheet(link, revision) {
  const next = link.cloneNode();
  const url = new URL(link.href);
  url.searchParams.set("deck-revision", revision);
  next.href = url;
  next.disabled = true;
  link.after(next);

  await new Promise((resolve, reject) => {
    next.addEventListener("load", resolve, { once: true });
    next.addEventListener("error", reject, { once: true });
  });

  next.disabled = false;
  link.remove();
}

async function reloadStyles(revision, path) {
  const links = [...document.querySelectorAll('link[rel="stylesheet"]')].filter((link) => {
    if (!link.href.startsWith(location.origin)) {
      return false;
    }
    if (!path) {
      return true;
    }
    const target = new URL(link.href).pathname;
    return target.endsWith("/@deck/design.css") || target.endsWith(`/${path}`);
  });

  await Promise.allSettled(links.map((link) => replaceStylesheet(link, revision)));
  post("style-reloaded", { revision });
}

/* -------------------------------------------------------------------------- */
/* parent protocol                                                             */
/* -------------------------------------------------------------------------- */

function handleMessage(message) {
  const payload = message.payload ?? {};
  switch (message.type) {
    case "set-step":
      applyStep(payload.step, {
        instant: Boolean(payload.instant),
        direction: payload.direction ?? "forward",
      });
      break;
    case "enter":
      state.visible = true;
      dispatch("deck:enter", { slideId: state.slideId, step: state.step, ...payload });
      break;
    case "leave":
      state.visible = false;
      dispatch("deck:leave", { slideId: state.slideId, step: state.step, ...payload });
      break;
    case "pause":
      dispatch("deck:pause", { slideId: state.slideId });
      break;
    case "resume":
      dispatch("deck:resume", { slideId: state.slideId });
      break;
    case "prepare-print":
      void preparePrint(payload);
      break;
    case "reload-style":
      void reloadStyles(payload.revision ?? Date.now(), payload.path);
      break;
    case "dispose":
      dispatch("deck:dispose", { slideId: state.slideId });
      break;
    case "ping":
      post("pong", { ready: state.ready });
      break;
    default:
      break;
  }
}

function installMessageListener() {
  window.addEventListener("message", (event) => {
    if (event.origin !== location.origin) {
      return;
    }
    if (event.source !== window.parent) {
      return;
    }
    const message = event.data;
    if (!message || message.namespace !== PROTOCOL.namespace || message.version !== PROTOCOL.version) {
      return;
    }
    if (message.slideId && state.slideId && message.slideId !== state.slideId) {
      return;
    }
    handleMessage(message);
  });
}

function installErrorReporting() {
  window.addEventListener("error", (event) => {
    if (event.target instanceof HTMLElement && event.target !== window) {
      const url = event.target.src || event.target.href;
      if (url) {
        addDiagnostic("missing-asset", "error", `リソースを読み込めませんでした: ${url}`, {
          selector: cssPath(event.target),
          url,
        });
        return;
      }
    }
    addDiagnostic("javascript-exception", "error", event.message, {
      url: event.filename,
      line: event.lineno,
      column: event.colno,
      stack: event.error?.stack,
    });
  }, true);

  window.addEventListener("unhandledrejection", (event) => {
    addDiagnostic("unhandled-rejection", "error", String(event.reason), {
      stack: event.reason?.stack,
    });
  });

  const originalError = console.error.bind(console);
  console.error = (...args) => {
    addDiagnostic("console-error", "error", args.map((value) => String(value)).join(" "));
    originalError(...args);
  };
}

/** Keyboard navigation when a slide is opened directly, outside the shell. */
function installStandaloneControls() {
  window.addEventListener("keydown", (event) => {
    if (event.metaKey || event.ctrlKey || event.altKey) {
      return;
    }
    switch (event.key) {
      case "ArrowRight":
      case "PageDown":
      case " ":
        event.preventDefault();
        applyStep(Math.min(state.step + 1, state.stepCount));
        break;
      case "ArrowLeft":
      case "PageUp":
        event.preventDefault();
        applyStep(Math.max(state.step - 1, 0), { direction: "backward" });
        break;
      case "Home":
        applyStep(0, { direction: "backward", instant: true });
        break;
      case "End":
        applyStep(state.stepCount, { instant: true });
        break;
      default:
        break;
    }
  });
}

/* -------------------------------------------------------------------------- */
/* public API                                                                  */
/* -------------------------------------------------------------------------- */

const deck = {
  get mode() {
    return state.mode;
  },
  get slideId() {
    return state.slideId;
  },
  get step() {
    return state.step;
  },
  get stepCount() {
    return state.stepCount;
  },
  get reducedMotion() {
    return state.reducedMotion;
  },
  get ready() {
    return state.ready;
  },
  get canvas() {
    return { ...env.canvas };
  },
  /** `{ index, number, total }` for this slide within the deck. */
  get position() {
    return { ...state.position };
  },
  /** Resolves once the slide's position in the deck is known. */
  whenPositioned: () => loadPosition(),

  /** Request an absolute step. The shell stays the source of truth. */
  goToStep(step) {
    if (embedded) {
      post("request-step", { step });
    } else {
      applyStep(step);
    }
  },
  next() {
    this.goToStep(state.step + 1);
  },
  previous() {
    this.goToStep(state.step - 1);
  },
  /** Request navigation to another slide (shell decides). */
  goToSlide(slideId, step = 0) {
    post("request-slide", { target: slideId, step });
  },

  /** Declare extra steps that are not expressed with [data-step]. */
  setStepCount(count) {
    declaredStepCount = Number(count) || 0;
    state.stepCount = computeStepCount();
    post("step-count", { stepCount: state.stepCount });
  },

  /** Delay `deck:ready` until the given promise settles. */
  waitUntil(promise) {
    readyPromises.push(Promise.resolve(promise));
  },

  /** Register a timeline so printing can seek it to its final state. */
  registerTimeline(timeline) {
    timelines.push(timeline);
    return timeline;
  },

  /** Resolve to the anime.js module, or null when animation is disabled. */
  anime: () => loadAnime(),

  /**
   * Resolve to `animate`, or null when animation is disabled.
   *
   * Printing and checking must be deterministic, so both get the final state
   * immediately instead of a frame from the middle of an animation.
   */
  animator: async () => {
    if (state.reducedMotion || state.mode === "print" || state.mode === "check") {
      return null;
    }
    const anime = await loadAnime();
    return anime?.animate ?? null;
  },

  get diagnostics() {
    return [...diagnostics];
  },
};

export function boot() {
  if (window.deck) {
    return window.deck;
  }
  Object.defineProperty(window, "deck", { value: deck, writable: false, enumerable: true });

  installErrorReporting();
  installMessageListener();
  if (!embedded) {
    installStandaloneControls();
  }

  void becomeReady();
  return deck;
}

export { deck, applyStep, replaceStylesheet };
