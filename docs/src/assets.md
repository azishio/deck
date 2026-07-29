# Assets, fonts and the cursor

Everything under `assets/` is served from `/assets/…`.

```text
assets/
├── images/       photographs, diagrams, screenshots
├── icons/        small SVGs used inline or as backgrounds
├── fonts/        webfonts, registered automatically
├── data/         JSON, CSV, anything a slide fetches at runtime
└── cursor.svg    optional custom mouse cursor
```

The subdirectories are a convention, not a mechanism — nothing breaks if you add
`assets/video/`. They exist so a deck with fifty files still has an obvious place to put
the fifty-first.

<div class="warning">

**Always reference assets with a root-absolute path**: `/assets/images/diagram.svg`.
`deck build` relocates each slide to `slides/<id>/index.html`, so a relative path that
works during `deck dev` will 404 in the static build. `deck check` reports these as
`missing_file` or `invalid_local_url`.

</div>

## Fonts

Drop font files into `assets/fonts/` and deck writes the `@font-face` rules. The file
name carries the metadata:

```text
Inter.woff2                    -> Inter, weight 400, normal
Inter-700.woff2                -> Inter, weight 700
Inter-SemiBold.woff2           -> Inter, weight 600
NotoSansJP-Bold-Italic.woff2   -> NotoSansJP, weight 700, italic
```

Recognised: `.woff2`, `.woff`, `.ttf`, `.otf`. Weights may be numeric (`100`–`1000`) or
named (`thin`, `light`, `regular`, `medium`, `semibold`, `bold`, `black`, …), and
`italic` or `oblique` sets the style. Subdirectories are walked, so
`assets/fonts/Inter/Inter-700.woff2` works too.

They load with `font-display: block`, because a slide should never flash its fallback in
front of an audience. Slide readiness waits for `document.fonts.ready`, so printing and
checking always see the real font.

Then point a token at the family:

```css
/* design/tokens.css */
:root {
  --deck-font-sans: "Inter", sans-serif;
  --deck-font-mono: "JetBrains Mono", ui-monospace, monospace;
}
```

`deck check` reports `missing_font` if a family is referenced but never loads — which
catches the classic "it works on my machine" font, and is why `deck doctor` lists the
families it can find.

## Custom cursor

Put `cursor.svg` — or `.png`, `.webp`, `.gif`, `.jpg` — directly in `assets/` and it
replaces the mouse cursor on every slide and across the presentation view.

```text
assets/cursor.svg
```

Two constraints come from the browser, not from deck: an SVG cursor needs explicit
`width` and `height` attributes, and images larger than 128×128 are ignored. A 24–32px
image with the hotspot at its top-left corner behaves the way people expect.

Override it per element with ordinary CSS:

```css
deck-code { cursor: text; }
```
