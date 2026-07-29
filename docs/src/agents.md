# Working with agents

deck's input format is plain HTML, which makes "add a slide about X" a request a coding
agent can carry out and you can review as a diff. There is no intermediate
representation to translate through, and no build output to inspect separately from the
source.

`deck init` writes three skills so the agent starts with the house rules instead of
guessing them:

```text
my-deck/
├── .agents/skills/                       ← the canonical copies
│   ├── deck-slides/SKILL.md
│   ├── deck-styling/SKILL.md
│   └── deck-components/SKILL.md
└── .claude/skills -> ../.agents/skills   ← symlink, so Claude Code finds them
```

One set of files, two paths. `.agents/skills/` is the vendor-neutral location; the
symlink means Claude Code picks them up without a second copy to keep in sync. On
Windows, where symlinks need elevation, `deck init` falls back to copies and says so.

## Three skills, three jobs

They are split by task, because "add a slide about X" and "make the accent colour warmer"
need different things in front of the agent — and a skill that tries to cover both ends
up too long to be read carefully.

**`deck-slides`** — writing and editing slides. The ground rules (one file is one
complete HTML document, the canvas is fixed at 1280×720, content goes in child elements,
assets use root-absolute paths), file conventions, a skeleton to copy, the built-in
components, the absolute step model, and what each check rule means when it fires.

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

## Why the checks matter more with an agent

An agent cannot see the slide. `deck check` is what closes that loop: overflow, clipped
text, contrast, broken assets and console errors all come back as text, with a selector
and a bounding box, which is exactly the feedback an agent can act on.

```bash
deck check --report json
```

So the useful instruction is not "make it look nice" but "make `deck check` pass", which
is checkable by both of you.

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
