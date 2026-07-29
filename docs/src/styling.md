# Styling and theming

Four project stylesheets, one Tailwind entry, and a cascade order that is fixed so you
never have to fight it.

| File | For | Loaded into |
|---|---|---|
| `design/tokens.css` | `--deck-*` values: sizes, fonts, spacing | `@layer project` |
| `design/theme.css` | the project look: colours, component tweaks | `@layer project` |
| `design/overrides.css` | the last word, for one-off fixes | `@layer project` |
| `design/tailwind.css` | `@theme`, `@utility`, `@apply` | Tailwind's own layers |

The first three are listed in `[theme].styles` in `deck.toml` and loaded in that order,
so `overrides.css` wins ties. They are all in the same layer; the split is a convention
for humans, not a mechanism.

## Start with a token

Every built-in style is driven by a custom property, so changing one value restyles the
whole deck:

```css
:root {
  --deck-color-accent: #4338ca;
  --deck-font-size-body: 26px;
  --deck-slide-padding-x: 80px;
}
```

The families are `--deck-color-*`, `--deck-font-*`, `--deck-font-size-*`,
`--deck-space-*`, `--deck-radius-*`, `--deck-slide-padding-*` and `--deck-step-*`. Run
`deck component show deck-card` to see which ones a component actually reads.

## Tailwind CSS

Tailwind is part of the runtime, not an optional add-on. The vendored v4 **browser
build** compiles utilities inside each slide document, so there is no build step, no
bundler and no network access.

deck's tokens are bridged into the Tailwind theme, which means the utilities follow your
theme instead of duplicating it:

```html
<p class="text-small text-muted font-mono">bg-surface, text-accent, rounded-card …</p>
```

Project additions go in `design/tailwind.css`:

```css
@theme {
  --color-brand: oklch(0.6 0.18 25);
}

@utility slide-lead {
  font-size: var(--deck-font-size-body);
  color: var(--deck-color-muted);
}
```

## The cascade

Declared in both `design.css` and the Tailwind entry, so it holds regardless of which
stylesheet the browser applies first:

```css
@layer base, theme, deck, project, slide, components, utilities;
@layer deck.reset, deck.tokens, deck.base, deck.components;
```

Reading left to right, lowest priority first:

1. **`base`** — Tailwind's preflight (the reset).
2. **`theme`** — Tailwind's theme variables.
3. **`deck`** — the deck design system: `reset`, `tokens`, `base`, `components`.
4. **`project`** — your `design/` files.
5. **`slide`** — per-slide CSS.
6. **`components`, `utilities`** — Tailwind's, which win over everything.

So your CSS outranks the design system, and a Tailwind utility outranks your CSS. That
is the intended order: utilities are the escape hatch for one element.

<div class="warning">

`deck.base` means "the `base` sub-layer of `deck`", which is **not** Tailwind's top-level
`base`. Listing them in a single `@layer` statement would place the whole `deck` layer
wherever its first sub-layer appears and sink the entire design system below preflight —
hence the two statements.

</div>

## Slide-local CSS

Wrap it so it stays in the right place:

```html
<style>
  @layer slide {
    #architecture deck-card { align-items: center; }
  }
</style>
```

Without the `@layer slide` wrapper the rule becomes unlayered, which puts it **above**
Tailwind's utilities — so a `class="text-accent"` on the same element would stop working.

## Taking a built-in apart

To edit a built-in component's styles rather than override them:

```bash
deck component eject deck-card    # -> design/ejected/deck-card.css
```

Add the file to `[theme].styles` and edit freely. The component keeps working; you have
simply moved its appearance into the project.

## Themes

`deck init --theme` picks the starting point for `design/theme.css`:

| | |
|---|---|
| `default` | light, red accent |
| `minimal-light` | light, blue accent, no shadows |
| `dark` | dark surfaces, light text |

It writes a normal file. There is no theme system to fight — after `init`, the theme is
just your CSS.
