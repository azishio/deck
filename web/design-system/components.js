// Built-in deck-* design system components.
//
// Light DOM is the default (see design doc 7.2). Shadow DOM is used only where
// hiding internal structure is genuinely valuable (deck-code).

const define = (name, ctor) => {
  if (!customElements.get(name)) {
    customElements.define(name, ctor);
  }
};

const escapeHtml = (value) =>
  value
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;");

/** Light DOM helper: keep exactly one generated child with the given class. */
function syncGeneratedChild(host, className, text, position) {
  let node = host.querySelector(`:scope > .${className}`);
  if (text === null || text === undefined || text === "") {
    node?.remove();
    return null;
  }
  if (!node) {
    node = document.createElement("span");
    node.className = className;
    node.dataset.deckGenerated = "";
    if (position === "start") {
      host.prepend(node);
    } else {
      host.append(node);
    }
  }
  if (node.textContent !== text) {
    node.textContent = text;
  }
  return node;
}

/* -------------------------------------------------------------------------- */
/* deck-slide                                                                  */
/* -------------------------------------------------------------------------- */

class DeckSlide extends HTMLElement {}
define("deck-slide", DeckSlide);

/* -------------------------------------------------------------------------- */
/* deck-heading                                                                */
/* -------------------------------------------------------------------------- */

class DeckHeading extends HTMLElement {
  static observedAttributes = ["eyebrow", "sub"];

  connectedCallback() {
    if (!this.hasAttribute("role")) {
      this.setAttribute("role", "heading");
    }
    if (!this.hasAttribute("aria-level")) {
      this.setAttribute("aria-level", this.getAttribute("level") === "title" ? "1" : "2");
    }
    this.#render();
  }

  attributeChangedCallback() {
    if (this.isConnected) {
      this.#render();
    }
  }

  #render() {
    syncGeneratedChild(this, "deck-heading__eyebrow", this.getAttribute("eyebrow"), "start");
    syncGeneratedChild(this, "deck-heading__sub", this.getAttribute("sub"), "end");
  }
}
define("deck-heading", DeckHeading);

/* -------------------------------------------------------------------------- */
/* deck-eyebrow / deck-title / deck-subtitle                                   */
/* -------------------------------------------------------------------------- */

// Styling-only elements. They exist so a slide can spell its structure out as
// children instead of packing it into deck-heading attributes, which is the
// house style for anything that is content rather than configuration.
class DeckEyebrow extends HTMLElement {}
define("deck-eyebrow", DeckEyebrow);

class DeckTitle extends HTMLElement {
  connectedCallback() {
    if (!this.hasAttribute("role")) {
      this.setAttribute("role", "heading");
    }
    if (!this.hasAttribute("aria-level")) {
      this.setAttribute("aria-level", "1");
    }
  }
}
define("deck-title", DeckTitle);

class DeckSubtitle extends HTMLElement {}
define("deck-subtitle", DeckSubtitle);

/* -------------------------------------------------------------------------- */
/* deck-footer / deck-slide-number / deck-progress                             */
/* -------------------------------------------------------------------------- */

class DeckFooter extends HTMLElement {}
define("deck-footer", DeckFooter);

/**
 * Renders this slide's position in the deck.
 *
 * `format` accepts a template with `{number}`, `{total}` and `{percent}`, so
 * `format="{number} / {total}"` (the default) or `format="{number}"` both work.
 */
class DeckSlideNumber extends HTMLElement {
  static observedAttributes = ["format"];

  #onReady = () => void this.#render();

  connectedCallback() {
    void this.#render();
    // Belt and braces: a component defined after boot still gets its number.
    document.addEventListener("deck:ready", this.#onReady, { once: true });
  }

  disconnectedCallback() {
    document.removeEventListener("deck:ready", this.#onReady);
  }

  attributeChangedCallback() {
    if (this.isConnected) {
      void this.#render();
    }
  }

  async #render() {
    const position = (await window.deck?.whenPositioned?.()) ?? { number: 0, total: 0 };
    if (position.total === 0) {
      this.textContent = "";
      return;
    }
    const percent = Math.round((position.number / position.total) * 100);
    this.textContent = (this.getAttribute("format") ?? "{number} / {total}")
      .replace("{number}", String(position.number))
      .replace("{total}", String(position.total))
      .replace("{percent}", `${percent}%`);
  }
}
define("deck-slide-number", DeckSlideNumber);

/** A thin bar showing how far through the deck this slide is. */
class DeckProgress extends HTMLElement {
  #onReady = () => void this.#render();

  connectedCallback() {
    void this.#render();
    document.addEventListener("deck:ready", this.#onReady, { once: true });
  }

  disconnectedCallback() {
    document.removeEventListener("deck:ready", this.#onReady);
  }

  async #render() {
    let bar = this.querySelector(":scope > .deck-progress__value");
    if (!bar) {
      bar = document.createElement("span");
      bar.className = "deck-progress__value";
      bar.dataset.deckGenerated = "";
      this.append(bar);
    }
    const position = (await window.deck?.whenPositioned?.()) ?? { number: 0, total: 0 };
    const ratio = position.total > 0 ? position.number / position.total : 0;
    bar.style.inlineSize = `${(ratio * 100).toFixed(2)}%`;
  }
}
define("deck-progress", DeckProgress);

/* -------------------------------------------------------------------------- */
/* deck-grid / deck-stack                                                      */
/* -------------------------------------------------------------------------- */

class DeckGrid extends HTMLElement {
  static observedAttributes = ["columns", "gap"];

  connectedCallback() {
    this.#render();
  }

  attributeChangedCallback() {
    if (this.isConnected) {
      this.#render();
    }
  }

  #render() {
    const columns = this.getAttribute("columns");
    this.style.setProperty("--deck-grid-columns", columns ?? "2");
    const gap = this.getAttribute("gap");
    if (gap) {
      this.style.setProperty("--deck-grid-gap", /^\d+$/.test(gap) ? `${gap}px` : gap);
    } else {
      this.style.removeProperty("--deck-grid-gap");
    }
  }
}
define("deck-grid", DeckGrid);

class DeckStack extends HTMLElement {
  static observedAttributes = ["gap"];

  connectedCallback() {
    this.#render();
  }

  attributeChangedCallback() {
    if (this.isConnected) {
      this.#render();
    }
  }

  #render() {
    const gap = this.getAttribute("gap");
    if (gap) {
      this.style.setProperty("--deck-stack-gap", /^\d+$/.test(gap) ? `${gap}px` : gap);
    } else {
      this.style.removeProperty("--deck-stack-gap");
    }
  }
}
define("deck-stack", DeckStack);

/* -------------------------------------------------------------------------- */
/* deck-card / deck-callout / deck-figure                                      */
/* -------------------------------------------------------------------------- */

class DeckCard extends HTMLElement {}
define("deck-card", DeckCard);

class DeckCallout extends HTMLElement {
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
    syncGeneratedChild(this, "deck-callout__label", this.getAttribute("label"), "start");
  }
}
define("deck-callout", DeckCallout);

class DeckFigure extends HTMLElement {
  static observedAttributes = ["caption"];

  connectedCallback() {
    this.#render();
  }

  attributeChangedCallback() {
    if (this.isConnected) {
      this.#render();
    }
  }

  #render() {
    syncGeneratedChild(this, "deck-figure__caption", this.getAttribute("caption"), "end");
  }
}
define("deck-figure", DeckFigure);

/* -------------------------------------------------------------------------- */
/* deck-stat                                                                   */
/* -------------------------------------------------------------------------- */

const NUMBER_IN_TEXT = /^(\D*?)(-?[\d,]+(?:\.\d+)?)(.*)$/s;

/**
 * `countup` animates the first child from zero.
 *
 * The animation is driven by `deck.onReveal`, so it runs when the stat becomes
 * visible and replays if you step away and back — exactly like the standard
 * reveal. `data-deck-countup` reflects the state (`idle`, `running`, `done`).
 */
class DeckStat extends HTMLElement {
  /** Parsed once, so an interrupted run cannot mistake a partial value for the target. */
  #target = null;
  #stopWatching = null;

  connectedCallback() {
    if (!this.hasAttribute("countup")) {
      return;
    }
    this.dataset.deckCountup = "idle";
    this.#stopWatching = window.deck?.onReveal?.(this, (reveal) => this.#countUp(reveal));
  }

  disconnectedCallback() {
    this.#stopWatching?.();
    this.#stopWatching = null;
  }

  /** Number, formatting and target element, read from the authored markup. */
  #resolveTarget() {
    if (this.#target) {
      return this.#target;
    }
    const element = this.firstElementChild;
    const parsed = element && NUMBER_IN_TEXT.exec(element.textContent ?? "");
    if (!parsed) {
      return null;
    }
    const [, prefix, rawNumber, suffix] = parsed;
    const value = Number.parseFloat(rawNumber.replace(/,/g, ""));
    if (!Number.isFinite(value)) {
      return null;
    }
    this.#target = {
      element,
      prefix,
      suffix,
      value,
      decimals: (rawNumber.split(".")[1] ?? "").length,
      grouped: rawNumber.includes(","),
    };
    return this.#target;
  }

  async #countUp({ signal }) {
    const target = this.#resolveTarget();
    if (!target) {
      return;
    }
    signal.addEventListener("abort", () => {
      this.dataset.deckCountup = "idle";
    });

    const render = (current) => {
      if (signal.aborted) {
        return;
      }
      const fixed = current.toFixed(target.decimals);
      const formatted = target.grouped
        ? Number(fixed).toLocaleString(undefined, {
            minimumFractionDigits: target.decimals,
            maximumFractionDigits: target.decimals,
          })
        : fixed;
      target.element.textContent = `${target.prefix}${formatted}${target.suffix}`;
    };

    const animate = await (window.deck?.animator?.() ?? Promise.resolve(null));
    if (signal.aborted) {
      return;
    }
    if (!animate) {
      render(target.value);
      this.dataset.deckCountup = "done";
      return;
    }

    this.dataset.deckCountup = "running";
    const state = { current: 0 };
    animate(state, {
      current: target.value,
      duration: Number(this.getAttribute("countup-duration") ?? 900),
      ease: "outExpo",
      onUpdate: () => render(state.current),
      onComplete: () => {
        render(target.value);
        if (!signal.aborted) {
          this.dataset.deckCountup = "done";
        }
      },
    });
  }
}
define("deck-stat", DeckStat);

/* -------------------------------------------------------------------------- */
/* deck-notes                                                                  */
/* -------------------------------------------------------------------------- */

class DeckNotes extends HTMLElement {}
define("deck-notes", DeckNotes);

/* -------------------------------------------------------------------------- */
/* deck-code (Shadow DOM)                                                      */
/* -------------------------------------------------------------------------- */

const LANGUAGES = {
  rust: {
    comment: String.raw`//[^\n]*|/\*[\s\S]*?\*/`,
    string: String.raw`r?"(?:[^"\\]|\\[\s\S])*"|'(?:[^'\\]|\\[\s\S])'`,
    keywords: [
      "as", "async", "await", "break", "const", "continue", "crate", "dyn", "else", "enum",
      "extern", "false", "fn", "for", "if", "impl", "in", "let", "loop", "match", "mod",
      "move", "mut", "pub", "ref", "return", "self", "Self", "static", "struct", "super",
      "trait", "true", "type", "unsafe", "use", "where", "while",
    ],
  },
  js: {
    comment: String.raw`//[^\n]*|/\*[\s\S]*?\*/`,
    string: String.raw`"(?:[^"\\]|\\[\s\S])*"|'(?:[^'\\]|\\[\s\S])*'|` + "`(?:[^`\\\\]|\\\\[\\s\\S])*`",
    keywords: [
      "async", "await", "break", "case", "catch", "class", "const", "continue", "default",
      "delete", "do", "else", "export", "extends", "finally", "for", "from", "function",
      "if", "import", "in", "instanceof", "let", "new", "null", "of", "return", "static",
      "super", "switch", "this", "throw", "true", "false", "try", "typeof", "undefined",
      "var", "void", "while", "yield",
    ],
  },
  python: {
    comment: String.raw`#[^\n]*`,
    string: String.raw`"""[\s\S]*?"""|'''[\s\S]*?'''|"(?:[^"\\]|\\[\s\S])*"|'(?:[^'\\]|\\[\s\S])*'`,
    keywords: [
      "and", "as", "assert", "async", "await", "break", "class", "continue", "def", "del",
      "elif", "else", "except", "False", "finally", "for", "from", "global", "if", "import",
      "in", "is", "lambda", "None", "nonlocal", "not", "or", "pass", "raise", "return",
      "True", "try", "while", "with", "yield",
    ],
  },
  go: {
    comment: String.raw`//[^\n]*|/\*[\s\S]*?\*/`,
    string: String.raw`"(?:[^"\\]|\\[\s\S])*"|` + "`[^`]*`",
    keywords: [
      "break", "case", "chan", "const", "continue", "default", "defer", "else", "fallthrough",
      "for", "func", "go", "goto", "if", "import", "interface", "map", "package", "range",
      "return", "select", "struct", "switch", "type", "var", "nil", "true", "false",
    ],
  },
  bash: {
    comment: String.raw`#[^\n]*`,
    string: String.raw`"(?:[^"\\]|\\[\s\S])*"|'[^']*'`,
    keywords: ["if", "then", "else", "elif", "fi", "for", "in", "do", "done", "case", "esac", "while", "function", "return", "export", "local"],
  },
  toml: {
    comment: String.raw`#[^\n]*`,
    string: String.raw`"""[\s\S]*?"""|"(?:[^"\\]|\\[\s\S])*"|'[^']*'`,
    keywords: ["true", "false"],
  },
  json: {
    comment: String.raw`(?!)`,
    string: String.raw`"(?:[^"\\]|\\[\s\S])*"`,
    keywords: ["true", "false", "null"],
  },
};

LANGUAGES.javascript = LANGUAGES.js;
LANGUAGES.typescript = LANGUAGES.js;
LANGUAGES.ts = LANGUAGES.js;
LANGUAGES.sh = LANGUAGES.bash;
LANGUAGES.shell = LANGUAGES.bash;
LANGUAGES.py = LANGUAGES.python;

function highlight(code, language) {
  const spec = LANGUAGES[language];
  if (!spec) {
    return escapeHtml(code);
  }

  const pattern = new RegExp(
    [
      `(?<comment>${spec.comment})`,
      `(?<string>${spec.string})`,
      String.raw`(?<number>\b\d[\d_]*(?:\.\d+)?(?:[eE][+-]?\d+)?\b)`,
      `(?<keyword>\\b(?:${spec.keywords.join("|")})\\b)`,
    ].join("|"),
    "g",
  );

  let html = "";
  let cursor = 0;
  for (const match of code.matchAll(pattern)) {
    html += escapeHtml(code.slice(cursor, match.index));
    const kind = Object.keys(match.groups).find((key) => match.groups[key] !== undefined);
    html += `<span class="tok tok-${kind}">${escapeHtml(match[0])}</span>`;
    cursor = match.index + match[0].length;
  }
  return html + escapeHtml(code.slice(cursor));
}

/** Remove the common leading indentation produced by HTML source formatting. */
function dedent(source) {
  const lines = source.replace(/\t/g, "  ").split("\n");
  while (lines.length > 0 && lines[0].trim() === "") {
    lines.shift();
  }
  while (lines.length > 0 && lines.at(-1).trim() === "") {
    lines.pop();
  }
  const indent = lines
    .filter((line) => line.trim() !== "")
    .reduce((min, line) => Math.min(min, line.length - line.trimStart().length), Infinity);
  return lines.map((line) => line.slice(Number.isFinite(indent) ? indent : 0)).join("\n");
}

const CODE_STYLE = new CSSStyleSheet();
CODE_STYLE.replaceSync(`
  :host {
    display: block;
    min-width: 0;
  }
  pre {
    margin: 0;
    padding: var(--deck-space-3, 24px);
    border-radius: var(--deck-radius-card, 16px);
    background: var(--deck-color-code-background, #1b2430);
    color: var(--deck-color-code-text, #e6edf3);
    font-family: var(--deck-font-mono, monospace);
    font-size: var(--deck-code-font-size, var(--deck-font-size-code, 20px));
    line-height: 1.55;
    overflow: hidden;
    tab-size: 2;
  }
  code {
    display: block;
    white-space: pre;
    font: inherit;
  }
  .tok-comment { color: var(--deck-code-comment, #8b98a5); font-style: italic; }
  .tok-string  { color: var(--deck-code-string, #9ece6a); }
  .tok-number  { color: var(--deck-code-number, #ff9e64); }
  .tok-keyword { color: var(--deck-code-keyword, #7aa2f7); }
  .line-highlight { display: block; background: var(--deck-code-highlight, rgb(122 162 247 / 18%)); }
`);

class DeckCode extends HTMLElement {
  static observedAttributes = ["language", "highlight-lines"];

  #source = "";

  connectedCallback() {
    if (!this.shadowRoot) {
      const root = this.attachShadow({ mode: "open" });
      root.adoptedStyleSheets = [CODE_STYLE];
      root.innerHTML = "<pre part='pre'><code part='code'></code></pre>";
      this.#source = dedent(this.textContent ?? "");
      this.textContent = "";
    }
    this.#render();
  }

  attributeChangedCallback() {
    if (this.shadowRoot) {
      this.#render();
    }
  }

  /** Replace the displayed source programmatically. */
  set code(value) {
    this.#source = dedent(String(value));
    this.#render();
  }

  get code() {
    return this.#source;
  }

  #render() {
    const code = this.shadowRoot.querySelector("code");
    const highlighted = highlight(this.#source, this.getAttribute("language") ?? "");
    const lines = this.#highlightedLines();
    if (lines.size === 0) {
      code.innerHTML = highlighted;
      return;
    }
    code.innerHTML = highlighted
      .split("\n")
      .map((line, index) =>
        lines.has(index + 1) ? `<span class="line-highlight">${line || " "}</span>` : line,
      )
      .join("\n");
  }

  #highlightedLines() {
    const spec = this.getAttribute("highlight-lines");
    const lines = new Set();
    if (!spec) {
      return lines;
    }
    for (const part of spec.split(",")) {
      const [from, to] = part.trim().split("-").map(Number);
      if (!Number.isFinite(from)) {
        continue;
      }
      for (let line = from; line <= (Number.isFinite(to) ? to : from); line += 1) {
        lines.add(line);
      }
    }
    return lines;
  }
}
define("deck-code", DeckCode);

export { define, escapeHtml, highlight, dedent };
