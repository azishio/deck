# Working with agents

deck's input format is plain HTML, which makes "add a slide about X" a request a coding
agent can carry out and you can review as a diff. There is no intermediate
representation to translate through, and no build output to inspect separately from the
source.

`deck init` writes a skill so the agent starts with the house rules instead of guessing
them:

```text
my-deck/
├── .agents/skills/deck-slides/SKILL.md   ← the canonical copy
└── .claude/skills -> ../.agents/skills   ← symlink, so Claude Code finds it
```

One file, two paths. `.agents/skills/` is the vendor-neutral location; the symlink means
Claude Code picks it up without a second copy to keep in sync. On Windows, where symlinks
need elevation, `deck init` falls back to a copy and says so.

## What the skill covers

The same ground as this guide, compressed to what an agent needs while editing:

- the ground rules — one file is one complete HTML document, the canvas is fixed at
  1280×720, content goes in child elements, assets use root-absolute paths, run
  `deck check` before finishing
- file conventions: order from the path, identity from `deck-slide[id]`, title from
  `<title>`, and `deck add slide --after` rather than inventing a number
- a slide skeleton to copy
- the built-in components and their attributes
- the absolute step model and the `deck:*` events
- where styling belongs: token, then Tailwind utility, then slide-local `@layer slide`
- how to add a component, and why never to animate from `connectedCallback`
- what each check rule means when it fires

## Why the checks matter more with an agent

An agent cannot see the slide. `deck check` is what closes that loop: overflow, clipped
text, contrast, broken assets and console errors all come back as text, with a selector
and a bounding box, which is exactly the feedback an agent can act on.

```bash
deck check --report json
```

So the useful instruction is not "make it look nice" but "make `deck check` pass", which
is checkable by both of you.

## Editing the skill

It is a normal file in your project — edit it. Deck-specific conventions belong there:
the fonts you use, the components you have added, the slide numbering scheme for a
hundred-slide deck, whatever your team keeps getting wrong.

The frontmatter decides when the skill is loaded, so keep the `description` concrete
about the situations it applies to:

```markdown
---
name: deck-slides
description: Author and edit slides in this deck. Use when adding or rewriting a slide
  under slides/, changing the deck's look through design/, fixing a `deck check`
  violation, or answering questions about how this deck is structured.
---
```
