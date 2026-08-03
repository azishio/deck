# Working with agents

deck's input format is plain HTML, which makes "add a slide about X" a request a coding
agent can carry out and you can review as a diff. There is no intermediate
representation to translate through, and no build output to inspect separately from the
source.

`deck init` writes four skills so the agent starts with the house rules instead of
guessing them:

```text
my-deck/
├── .agents/skills/                       ← the canonical copies
│   ├── deck-slides/SKILL.md
│   ├── deck-visuals/SKILL.md
│   ├── deck-styling/SKILL.md
│   └── deck-components/SKILL.md
└── .claude/skills -> ../.agents/skills   ← symlink, so Claude Code finds them
```

One set of files, two paths. `.agents/skills/` is the vendor-neutral location; the
symlink means Claude Code picks them up without a second copy to keep in sync. On
Windows, where symlinks need elevation, `deck init` falls back to copies and says so.

## Four skills, four jobs

They are split by task, because "add a slide about X" and "make the accent colour warmer"
need different things in front of the agent — and a skill that tries to cover both ends
up too long to be read carefully.

**`deck-slides`** — writing and editing slides. The ground rules (one file is one
complete HTML document, the canvas is fixed at 1280×720, content goes in child elements,
assets use root-absolute paths), file conventions, a skeleton to copy, the built-in
components, the absolute step model, and what each check rule means when it fires.

**`deck-visuals`** — explaining something with a drawing that moves: inline SVG scenes
driven by `data-step` or by an `apply(step)` of their own, Anime.js and its SVG helpers in
the exact shapes they expect, canvas, and generating artwork rather than sourcing it.
Without it an agent defaults to three cards of prose, which is what a slide tool with no
HTML would have produced anyway.

**`deck-styling`** — how the deck looks. Which of the four `design/` files to reach for,
the token families, the Tailwind entry and the cascade order, webfonts in
`assets/fonts/`, the custom cursor, ejecting a built-in's styles, and the checks that
styling breaks.

**`deck-components`** — Custom Elements. The project prefix, Light DOM by default,
idempotent `connectedCallback`, styling from `design/` rather than JavaScript, the
runtime API, and why an animation must be tied to `deck.onReveal` rather than to
construction.

Each one names the others, so an agent that lands on the wrong skill is pointed at the
right one rather than guessing.

## Looking at a slide

A slide is a URL on localhost, so an agent with browser access can open it, read the DOM,
evaluate JavaScript against it and screenshot it — the same loop a person has, without
waiting to be told what went wrong.

Start the server on a fixed port and open one slide on its own:

```bash
deck dev --port 5173 --open none
```

```text
http://127.0.0.1:5173/slides/architecture
```

A single slide page has no presenter chrome, and the document **is** the canvas at
exactly 1280×720 — so a 1280×720 viewport captures the slide and nothing else. Query
parameters put it in a known state:

| | |
|---|---|
| `?step=2` | a specific step |
| `?step=final` | everything revealed |
| `?deck-mode=check` | animation disabled, so a screenshot is deterministic |

Wait for readiness before measuring anything, or you will screenshot a half-laid-out
document:

```js
document.documentElement.dataset.deckReady === "true"
```

Then the runtime is available for questions the DOM alone will not answer:

```js
window.deck.step;              // 2
window.deck.stepCount;         // 3
window.deck.position;          // { index: 3, number: 4, total: 18 }
window.deck.diagnostics;       // anything the slide reported
```

`/present` works too, but the slide lives inside an iframe there, so reach it through
`window.deckShell.currentFrame().iframe.contentDocument`. `window.deckShell` also drives
navigation — `goToSlideId("architecture")`, `setStep(2)`, `next()` — which is how the
end-to-end tests in this repository check hot reload and step behaviour.

## Why `deck check` still matters

Being able to look at a slide does not make the checks redundant; they answer a different
question.

```bash
deck check --report json
deck check --screenshots       # PNG per slide in .deck/screenshots/
```

- **It is exhaustive.** Eighteen slides, every rule, without deciding which ones to look
  at.
- **It is measured, not judged.** `contrast 4.36 < 4.5` and `+12px` are facts. "Looks a
  bit tight" is not something to act on with confidence.
- **It is reproducible.** Viewport, locale, timezone, device scale and reduced motion are
  pinned and animation is off, so two runs agree and a fix can be verified.
- **It catches what rendering hides.** A console error, a font that silently fell back, a
  request that left localhost — all invisible in a screenshot that looks fine.

So the useful instruction is "make `deck check` pass, then look at it" — the checks for
the facts, the browser for the judgement.

## Editing them

They are normal files in your project — edit them. Deck-specific conventions belong
there: the fonts you use, the components you have added, the slide numbering scheme for a
hundred-slide deck, whatever your team keeps getting wrong.

The frontmatter decides when a skill is loaded, so keep each `description` concrete about
the situations it applies to, and distinct from its siblings:

```markdown
---
name: deck-styling
description: Change how this deck looks. Use when adjusting colours, fonts, sizes or
  spacing, editing anything under design/, adding a webfont or a custom cursor to
  assets/, switching theme, writing Tailwind utilities, or fixing a low_contrast or
  min_font_size check.
---
```
