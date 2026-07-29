// Shared presentation shell: iframe ring, absolute step navigation, URL state,
// websocket synchronisation and hot reload.
//
// Used by both /present and /presenter. Frames are never reparented (that would
// reload the iframe); instead every frame slot lives in one absolutely
// positioned layer and is placed over a region rectangle measured from the page.

const PROTOCOL = Object.freeze({ namespace: "deck", version: 1 });

const clamp = (value, min, max) => Math.min(Math.max(value, min), max);

function randomId() {
  return Math.random().toString(36).slice(2, 10);
}

class Frame {
  constructor(shell, slide) {
    this.shell = shell;
    this.slide = slide;
    this.ready = false;
    this.meta = { stepCount: slide.stepCount ?? 0, notes: "", title: slide.title, tags: [], resources: [] };
    this.diagnostics = [];
    this.offset = null;
    this.region = null;

    this.slot = document.createElement("div");
    this.slot.className = "deck-frame-slot";
    this.slot.dataset.slideId = slide.id;

    this.iframe = shell.createIframe(slide);
    this.slot.append(this.iframe);
    shell.layer.append(this.slot);
  }

  get window() {
    return this.iframe.contentWindow;
  }

  owns(source) {
    return source === this.iframe.contentWindow || source === this.replacement?.contentWindow;
  }

  post(type, payload = {}) {
    this.window?.postMessage(
      { ...PROTOCOL, type, slideId: this.slide.id, payload },
      location.origin,
    );
  }

  destroy() {
    this.post("dispose");
    this.slot.remove();
  }
}

export class DeckShell {
  constructor(options) {
    this.base = options.base ?? "/";
    this.mode = options.mode ?? "present";
    this.layer = options.layer;
    this.preload = options.preload ?? 1;
    this.regionForOffset = options.regionForOffset ?? ((offset) => (offset === 0 ? "current" : null));
    this.regionRect = options.regionRect;
    this.canvas = options.canvas ?? { width: 1280, height: 720 };
    this.hotReload = options.hotReload ?? true;
    this.listeners = new Map();

    this.clientId = randomId();
    this.manifest = { revision: 0, slides: [] };
    this.frames = new Map();
    this.index = 0;
    this.step = 0;
    this.blackout = false;
    this.paused = false;
    this.applyingRemote = false;
    this.socket = null;
    this.socketRetry = 0;
  }

  /* -- events ----------------------------------------------------------- */

  on(type, handler) {
    const handlers = this.listeners.get(type) ?? new Set();
    handlers.add(handler);
    this.listeners.set(type, handlers);
    return () => handlers.delete(handler);
  }

  emit(type, detail) {
    for (const handler of this.listeners.get(type) ?? []) {
      handler(detail);
    }
  }

  /* -- lifecycle -------------------------------------------------------- */

  async start() {
    await this.loadManifest();

    const fromHash = this.readHash();
    if (fromHash) {
      this.index = fromHash.index;
      this.step = fromHash.step;
    }

    window.addEventListener("message", (event) => this.handleFrameMessage(event));
    window.addEventListener("hashchange", () => this.applyHash());
    window.addEventListener("resize", () => this.layoutFrames());
    if (this.hotReload || this.mode !== "print") {
      this.connectSocket();
    }

    this.syncFrames({ initial: true });
    this.writeHash();
    this.emit("change", this.snapshot());
  }

  async loadManifest() {
    const response = await fetch(`${this.base}@deck/manifest.json`, { cache: "no-store" });
    if (!response.ok) {
      throw new Error(`manifest を取得できません: ${response.status}`);
    }
    this.manifest = await response.json();
    this.emit("manifest", this.manifest);
  }

  get slides() {
    return this.manifest.slides;
  }

  get slide() {
    return this.slides[this.index];
  }

  snapshot() {
    return {
      index: this.index,
      step: this.step,
      slide: this.slide,
      total: this.slides.length,
      stepCount: this.stepCountAt(this.index),
      blackout: this.blackout,
      paused: this.paused,
      frame: this.slide ? this.frames.get(this.slide.id) : null,
    };
  }

  stepCountAt(index) {
    const slide = this.slides[index];
    if (!slide) {
      return 0;
    }
    const frame = this.frames.get(slide.id);
    return frame?.ready ? frame.meta.stepCount : (slide.stepCount ?? 0);
  }

  /* -- navigation ------------------------------------------------------- */

  next() {
    if (this.step < this.stepCountAt(this.index)) {
      this.setStep(this.step + 1);
    } else if (this.index + 1 < this.slides.length) {
      this.goTo(this.index + 1, 0, { direction: "forward" });
    }
  }

  previous() {
    if (this.step > 0) {
      this.setStep(this.step - 1, { direction: "backward" });
    } else if (this.index > 0) {
      this.goTo(this.index - 1, this.stepCountAt(this.index - 1), { direction: "backward" });
    }
  }

  nextSlide() {
    this.goTo(this.index + 1, 0, { direction: "forward" });
  }

  previousSlide() {
    this.goTo(this.index - 1, 0, { direction: "backward" });
  }

  first() {
    this.goTo(0, 0, { instant: true });
  }

  last() {
    this.goTo(this.slides.length - 1, 0, { instant: true });
  }

  setStep(step, { direction = "forward", instant = false } = {}) {
    const target = clamp(step, 0, this.stepCountAt(this.index));
    if (target === this.step) {
      return;
    }
    this.step = target;
    this.currentFrame()?.post("set-step", { step: target, direction, instant });
    this.writeHash();
    this.publish();
    this.emit("change", this.snapshot());
  }

  goTo(index, step = 0, { direction = "forward", instant = false } = {}) {
    const target = clamp(index, 0, Math.max(this.slides.length - 1, 0));
    if (target === this.index) {
      this.setStep(step, { direction, instant });
      return;
    }
    this.currentFrame()?.post("leave", { direction });
    this.index = target;
    this.step = step;
    this.syncFrames({ direction });
    this.writeHash();
    this.publish();
    this.emit("change", this.snapshot());
  }

  goToSlideId(slideId, step = 0) {
    const index = this.slides.findIndex((slide) => slide.id === slideId);
    if (index >= 0) {
      this.goTo(index, step, { instant: true });
    }
  }

  currentFrame() {
    return this.slide ? this.frames.get(this.slide.id) : null;
  }

  /* -- frame ring ------------------------------------------------------- */

  offsets() {
    const range = [];
    for (let offset = -this.preload; offset <= this.preload; offset += 1) {
      range.push(offset);
    }
    // Regions explicitly requested by the page must always be materialised.
    for (const offset of [0, 1]) {
      if (!range.includes(offset) && this.regionForOffset(offset)) {
        range.push(offset);
      }
    }
    return range.sort((a, b) => Math.abs(a) - Math.abs(b));
  }

  createIframe(slide) {
    const iframe = document.createElement("iframe");
    iframe.className = "deck-frame";
    iframe.title = slide.title ?? slide.id;
    iframe.setAttribute("loading", "eager");
    iframe.src = this.slideUrl(slide);
    return iframe;
  }

  slideUrl(slide, extra = {}) {
    const url = new URL(`${this.base}slides/${slide.path.replace(/\.html$/, "")}`, location.href);
    url.pathname = `${this.base}slides/${slide.id}`.replace(/\/{2,}/g, "/");
    url.searchParams.set("deck-mode", this.mode);
    for (const [key, value] of Object.entries(extra)) {
      url.searchParams.set(key, String(value));
    }
    return url.toString();
  }

  syncFrames({ direction = "forward", initial = false } = {}) {
    const wanted = new Map();
    for (const offset of this.offsets()) {
      const slide = this.slides[this.index + offset];
      if (slide && !wanted.has(slide.id)) {
        wanted.set(slide.id, offset);
      }
    }

    for (const [slideId, frame] of this.frames) {
      if (!wanted.has(slideId)) {
        frame.destroy();
        this.frames.delete(slideId);
      }
    }

    for (const [slideId, offset] of wanted) {
      let frame = this.frames.get(slideId);
      if (!frame) {
        const slide = this.slides.find((entry) => entry.id === slideId);
        frame = new Frame(this, slide);
        this.frames.set(slideId, frame);
      }
      frame.offset = offset;
      frame.region = this.regionForOffset(offset);
    }

    this.layoutFrames();

    const current = this.currentFrame();
    if (current?.ready) {
      current.post("enter", { direction, instant: initial });
      current.post("set-step", { step: this.step, instant: true });
    }
  }

  layoutFrames() {
    const fallback = this.regionRect?.("current");
    for (const frame of this.frames.values()) {
      const rect = frame.region ? this.regionRect?.(frame.region) : null;
      const box = rect ?? fallback;
      const visible = Boolean(rect);
      frame.slot.dataset.region = frame.region ?? "hidden";
      frame.slot.dataset.offset = String(frame.offset);
      frame.slot.classList.toggle("is-visible", visible);
      if (!box) {
        continue;
      }
      const scale = Math.min(box.width / this.canvas.width, box.height / this.canvas.height);
      const left = box.left + (box.width - this.canvas.width * scale) / 2;
      const top = box.top + (box.height - this.canvas.height * scale) / 2;
      frame.slot.style.width = `${this.canvas.width}px`;
      frame.slot.style.height = `${this.canvas.height}px`;
      frame.slot.style.transform = `translate(${left}px, ${top}px) scale(${scale})`;
      frame.slot.style.setProperty("--deck-scale", String(scale));
    }
  }

  /* -- frame messages --------------------------------------------------- */

  frameFor(source) {
    for (const frame of this.frames.values()) {
      if (frame.owns(source)) {
        return frame;
      }
    }
    return null;
  }

  handleFrameMessage(event) {
    if (event.origin !== location.origin) {
      return;
    }
    const message = event.data;
    if (!message || message.namespace !== PROTOCOL.namespace || message.version !== PROTOCOL.version) {
      return;
    }
    const frame = this.frameFor(event.source);
    if (!frame) {
      return;
    }
    const fromReplacement = event.source === frame.replacement?.contentWindow;
    const payload = message.payload ?? {};

    switch (message.type) {
      case "ready":
        if (fromReplacement) {
          void this.promoteReplacement(frame, payload);
          return;
        }
        frame.ready = true;
        frame.meta = { ...frame.meta, ...payload };
        frame.diagnostics = payload.diagnostics ?? [];
        if (frame === this.currentFrame()) {
          frame.post("enter", { instant: true });
          frame.post("set-step", { step: this.step, instant: true });
        }
        this.emit("frame-ready", { frame, payload });
        this.emit("change", this.snapshot());
        break;
      case "step-changed":
        if (frame === this.currentFrame()) {
          frame.meta.stepCount = payload.stepCount ?? frame.meta.stepCount;
          this.emit("change", this.snapshot());
        }
        break;
      case "step-count":
        frame.meta.stepCount = payload.stepCount ?? frame.meta.stepCount;
        this.emit("change", this.snapshot());
        break;
      case "request-step":
        if (frame === this.currentFrame()) {
          this.setStep(payload.step);
        }
        break;
      case "request-slide":
        this.goToSlideId(payload.target, payload.step ?? 0);
        break;
      case "diagnostic":
        frame.diagnostics.push(payload);
        this.emit("diagnostic", { frame, diagnostic: payload });
        break;
      case "print-ready":
        this.emit("print-ready", { frame, payload });
        break;
      default:
        break;
    }
  }

  /* -- hot reload ------------------------------------------------------- */

  connectSocket() {
    const url = new URL(`${this.base}ws`.replace(/\/{2,}/g, "/"), location.href);
    url.protocol = url.protocol === "https:" ? "wss:" : "ws:";
    let socket;
    try {
      socket = new WebSocket(url);
    } catch {
      return;
    }
    this.socket = socket;

    socket.addEventListener("open", () => {
      this.socketRetry = 0;
      this.emit("connection", { connected: true });
      socket.send(JSON.stringify({ type: "hello", clientId: this.clientId, role: this.mode }));
    });

    socket.addEventListener("message", (event) => {
      let message;
      try {
        message = JSON.parse(event.data);
      } catch {
        return;
      }
      this.handleServerMessage(message);
    });

    socket.addEventListener("close", () => {
      this.emit("connection", { connected: false });
      this.socketRetry += 1;
      setTimeout(() => this.connectSocket(), Math.min(1000 * this.socketRetry, 5000));
    });

    socket.addEventListener("error", () => socket.close());
  }

  publish() {
    if (this.applyingRemote || this.socket?.readyState !== WebSocket.OPEN) {
      return;
    }
    this.socket.send(
      JSON.stringify({
        type: "sync",
        clientId: this.clientId,
        state: {
          slideId: this.slide?.id ?? null,
          step: this.step,
          blackout: this.blackout,
          paused: this.paused,
          ...(this.extraSyncState?.() ?? {}),
        },
      }),
    );
  }

  applySync(state) {
    if (!state) {
      return;
    }
    this.applyingRemote = true;
    try {
      if (state.slideId && state.slideId !== this.slide?.id) {
        const index = this.slides.findIndex((slide) => slide.id === state.slideId);
        if (index >= 0) {
          this.goTo(index, state.step ?? 0, { instant: true });
        }
      } else if (typeof state.step === "number" && state.step !== this.step) {
        this.setStep(state.step, { instant: true });
      }
      if (typeof state.blackout === "boolean" && state.blackout !== this.blackout) {
        this.setBlackout(state.blackout);
      }
      if (typeof state.paused === "boolean") {
        this.paused = state.paused;
      }
      this.emit("remote-state", state);
    } finally {
      this.applyingRemote = false;
    }
  }

  handleServerMessage(message) {
    switch (message.type) {
      case "hello":
        this.manifestRevision = message.revision;
        if (message.sync) {
          this.applySync(message.sync.state);
        }
        break;
      case "sync":
        if (message.clientId !== this.clientId) {
          this.applySync(message.state);
        }
        break;
      case "slide-changed":
        this.reloadSlide(message.slideId, message.revision);
        break;
      case "style-changed":
        for (const frame of this.frames.values()) {
          frame.post("reload-style", { revision: message.revision, path: message.path });
        }
        this.emit("hot", message);
        break;
      case "tailwind-changed":
        // The Tailwind entry is inlined into each slide document, so the
        // frames have to be re-fetched rather than restyled.
        for (const frame of this.frames.values()) {
          this.reloadFrame(frame, message.revision);
        }
        this.emit("hot", message);
        break;
      case "component-changed":
        for (const frame of this.frames.values()) {
          const tags = message.tags ?? [];
          const used = tags.length === 0 || tags.some((tag) => frame.meta.tags?.includes(tag));
          if (used) {
            this.reloadFrame(frame, message.revision);
          }
        }
        this.emit("hot", message);
        break;
      case "asset-changed":
        for (const frame of this.frames.values()) {
          const uses = (frame.meta.resources ?? []).some((url) => url.includes(message.path));
          if (uses) {
            this.reloadFrame(frame, message.revision);
          }
        }
        this.emit("hot", message);
        break;
      case "manifest-changed":
        void this.refreshManifest(message);
        break;
      case "config-changed":
        location.reload();
        break;
      case "error":
        this.emit("server-error", message);
        break;
      default:
        break;
    }
  }

  reloadSlide(slideId, revision) {
    const frame = this.frames.get(slideId);
    if (frame) {
      this.reloadFrame(frame, revision);
    }
    this.emit("hot", { type: "slide-changed", slideId, revision });
  }

  /** Double-buffered reload: keep the old iframe visible until the new one is ready. */
  reloadFrame(frame, revision) {
    if (frame.replacement) {
      frame.replacement.remove();
    }
    const replacement = this.createIframe(frame.slide);
    replacement.classList.add("deck-frame-replacement");
    replacement.src = this.slideUrl(frame.slide, { "deck-revision": revision ?? Date.now() });
    frame.replacement = replacement;
    frame.slot.append(replacement);
  }

  async promoteReplacement(frame, payload) {
    const replacement = frame.replacement;
    if (!replacement) {
      return;
    }
    frame.meta = { ...frame.meta, ...payload };
    frame.diagnostics = payload.diagnostics ?? [];

    const step = frame === this.currentFrame() ? this.step : 0;
    replacement.contentWindow?.postMessage(
      { ...PROTOCOL, type: "set-step", slideId: frame.slide.id, payload: { step, instant: true } },
      location.origin,
    );
    if (frame === this.currentFrame()) {
      replacement.contentWindow?.postMessage(
        { ...PROTOCOL, type: "enter", slideId: frame.slide.id, payload: { instant: true } },
        location.origin,
      );
    }

    await new Promise((resolve) => {
      requestAnimationFrame(() => resolve());
      setTimeout(resolve, 50);
    });
    replacement.classList.add("is-active");
    frame.iframe.classList.add("is-retiring");

    const previous = frame.iframe;
    frame.iframe = replacement;
    frame.replacement = null;
    frame.ready = true;
    replacement.classList.remove("deck-frame-replacement");

    setTimeout(() => previous.remove(), 200);
    this.emit("frame-ready", { frame, payload, hot: true });
    this.emit("change", this.snapshot());
  }

  async refreshManifest(message) {
    const previousId = this.slide?.id;
    const previousIndex = this.index;
    await this.loadManifest();

    let index = this.slides.findIndex((slide) => slide.id === previousId);
    if (index < 0) {
      index = clamp(previousIndex, 0, Math.max(this.slides.length - 1, 0));
      this.step = 0;
    }
    this.index = index;

    // Drop frames whose slide disappeared or moved to a different file.
    for (const [slideId, frame] of this.frames) {
      const slide = this.slides.find((entry) => entry.id === slideId);
      if (!slide) {
        frame.destroy();
        this.frames.delete(slideId);
      }
    }

    this.syncFrames({ initial: true });
    this.writeHash();
    this.emit("hot", message ?? { type: "manifest-changed" });
    this.emit("change", this.snapshot());
  }

  /* -- presentation state ----------------------------------------------- */

  setBlackout(value) {
    this.blackout = value;
    document.documentElement.dataset.deckBlackout = String(value);
    this.publish();
    this.emit("change", this.snapshot());
  }

  toggleBlackout() {
    this.setBlackout(!this.blackout);
  }

  /* -- URL state -------------------------------------------------------- */

  readHash() {
    const match = /^#\/([^/]+)(?:\/(\d+))?$/.exec(location.hash);
    if (!match) {
      return null;
    }
    const slideId = decodeURIComponent(match[1]);
    const index = this.slides.findIndex((slide) => slide.id === slideId);
    if (index < 0) {
      return null;
    }
    return { index, step: Number.parseInt(match[2] ?? "0", 10) || 0 };
  }

  applyHash() {
    const parsed = this.readHash();
    if (!parsed) {
      return;
    }
    if (parsed.index !== this.index) {
      this.goTo(parsed.index, parsed.step, { instant: true });
    } else if (parsed.step !== this.step) {
      this.setStep(parsed.step, { instant: true });
    }
  }

  writeHash() {
    if (!this.slide) {
      return;
    }
    const hash = `#/${encodeURIComponent(this.slide.id)}/${this.step}`;
    if (location.hash !== hash) {
      history.replaceState(null, "", hash);
    }
  }
}

export function installKeyboard(shell, extra = {}) {
  window.addEventListener("keydown", (event) => {
    if (event.metaKey || event.ctrlKey || event.altKey) {
      return;
    }
    const target = event.target;
    if (target instanceof HTMLElement && /^(INPUT|TEXTAREA|SELECT)$/.test(target.tagName)) {
      return;
    }

    const handler = extra[event.key];
    if (handler) {
      event.preventDefault();
      handler(event);
      return;
    }

    switch (event.key) {
      case "ArrowRight":
      case "PageDown":
      case " ":
      case "Enter":
        event.preventDefault();
        shell.next();
        break;
      case "ArrowLeft":
      case "PageUp":
      case "Backspace":
        event.preventDefault();
        shell.previous();
        break;
      case "ArrowDown":
        event.preventDefault();
        shell.nextSlide();
        break;
      case "ArrowUp":
        event.preventDefault();
        shell.previousSlide();
        break;
      case "Home":
        event.preventDefault();
        shell.first();
        break;
      case "End":
        event.preventDefault();
        shell.last();
        break;
      case "b":
      case ".":
        event.preventDefault();
        shell.toggleBlackout();
        break;
      case "f":
        event.preventDefault();
        if (document.fullscreenElement) {
          void document.exitFullscreen();
        } else {
          void document.documentElement.requestFullscreen();
        }
        break;
      default:
        break;
    }
  });
}

export { PROTOCOL };
