# deck

**One slide is one complete HTML document.**

`deck` is a local slide runtime and development environment written in Rust. Slides are
plain HTML, CSS and JavaScript — there is no slide DSL, no required Markdown, and no
slide body hidden inside TOML or JSON. The design system is delivered as Web Components,
CSS Custom Properties and Tailwind CSS, all of which you are free to override, extend or
ignore.

📊 **[See the introduction deck](https://azishio.github.io/deck/present)** — it is built
by `deck build` from [`site/`](site) and published to GitHub Pages.

```bash
git clone https://github.com/azishio/deck
cd deck
cargo install --path crates/deck-cli

deck init my-deck
cd my-deck
deck dev
```

Requirements: a Rust toolchain to build the CLI, and a Chromium-based browser. **Node.js
is not required** — Tailwind CSS and Anime.js are vendored into the binary.

---

## Why

Most slide tools ask you to learn their format. This one asks you to use the web
platform you already know:

- **Editable by humans and agents alike.** A slide is a file you can read, diff and
  review. No intermediate representation.
- **Nothing is off-limits.** SVG, Canvas, WebGL, a live iframe, a custom element you
  wrote this morning — if it runs in Chromium, it runs in a slide.
- **Isolated by construction.** Each slide renders in its own `iframe`, so one slide's
  JavaScript, CSS and DOM state cannot leak into the next.
- **Verifiable.** Layout overflow, clipped text, broken assets and console errors are
  lint rules, not things you notice from the back of the room.

## Features

| | |
|---|---|
| **Directory-based discovery** | `slides/**/*.html` in lexicographic order. No slide list to maintain. |
| **Stable identity** | Order comes from the path, identity from `deck-slide[id]`, title from `<title>`. |
| **iframe isolation** | Per-slide JavaScript, CSS and DOM state, with a three-frame ring so 100 slides do not mean 100 iframes. |
| **Absolute steps** | `screen state = f(slide_id, step)`. Deep links and reloads land exactly where you left off. |
| **Tailwind CSS** | v4 browser build, vendored. Utilities compile inside each slide — no bundler, no CDN. |
| **Anime.js** | Vendored and importable straight from a slide. |
| **Hot reload** | HTML, CSS and component edits apply while keeping the current slide, step and presenter timer. |
| **Presenter View** | Current and next slide, speaker notes, timer, clock, blackout, live diagnostics. |
| **Print** | A real `/print` page for the browser's own print dialog. The CLI never generates a PDF. |
| **Checks** | headless Chromium finds layout, runtime and asset problems. Human, JSON or SARIF output. |
| **Static build** | `deck build` emits a folder any static host can serve. |

## Project layout

```text
my-deck/
├── deck.toml          # deck-wide configuration
├── deck.local.toml    # machine-specific overrides (not committed)
├── deck.lock          # versions of the bundled runtime
├── slides/            # one file per slide
├── components/        # your own Custom Elements
├── design/            # tokens.css / theme.css / overrides.css / tailwind.css
├── assets/            # images, fonts, data
├── dist/              # output of `deck build`
└── .deck/             # cache, reports, screenshots
```

The names `slides/`, `components/`, `design/` and `assets/` are fixed by convention and
cannot be reconfigured. Keeping the layout predictable keeps the CLI, the watcher, the
static build and any agent editing the deck simple.

## Writing a slide

```html
<!doctype html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <title>Architecture</title>

  <link rel="stylesheet" href="/@deck/design.css">
  <script type="module" src="/@deck/boot.js"></script>
</head>
<body>
  <deck-slide id="architecture">
    <deck-heading eyebrow="Architecture">
      One pipeline, end to end
    </deck-heading>

    <deck-grid columns="2" class="grow">
      <deck-card data-step="1">
        <h2 class="text-body font-bold">Collection</h2>
        <p>Probes push measurements into the ingest tier.</p>
      </deck-card>

      <deck-card data-step="2" variant="accent">
        <h2 class="text-body font-bold">Visualization</h2>
        <p>Queries fan out to the visualisation layer.</p>
      </deck-card>
    </deck-grid>

    <deck-notes>Explain the ingest path and the query path separately.</deck-notes>
  </deck-slide>
</body>
</html>
```

- `data-step="N"` reveals an element at step *N*. Steps are absolute, never relative.
- The `<link>` and `<script>` tags are injected automatically if you leave them out.
- Content goes in child elements. Large JSON blobs in attributes are a DSL in disguise —
  don't.

### Built-in components

`deck-slide` · `deck-heading` · `deck-grid` · `deck-stack` · `deck-card` ·
`deck-callout` · `deck-stat` · `deck-figure` · `deck-code` · `deck-notes`

Everything except `deck-code` renders in **Light DOM**, so ordinary CSS selectors,
Tailwind utilities, Anime.js and DevTools all keep working. `deck-*` is reserved; give
your own components a project-specific prefix.

```bash
deck component list
deck component show deck-card      # print the built-in styles
deck component eject deck-card     # copy them into design/ejected/
deck component new acme-metric     # scaffold a component and register it
```

### Tailwind CSS

Tailwind is a required part of the runtime. `/@deck/vendor/tailwind.js` (the Tailwind v4
browser build) compiles utilities inside each slide document, so there is no build step
and no network access.

- `design/tailwind.css` is the entry point. Put `@theme`, `@utility` and `@apply` there.
- deck's tokens are bridged into the Tailwind theme, so `bg-surface`, `text-accent`,
  `font-mono` and `rounded-card` follow your theme instead of duplicating it.
- The reset (Tailwind's preflight) is imported first. Set `[tailwind] preflight = false`
  to drop it.

The cascade order is declared in both `design.css` and the Tailwind entry:

```css
@layer base, theme, deck, project, slide, components, utilities;
@layer deck.reset, deck.tokens, deck.base, deck.components;
```

Reading left to right: Tailwind's reset, Tailwind's theme, the deck design system, your
project CSS, per-slide CSS, then Tailwind's utilities — which win over everything.

> `deck.base` means "the `base` sub-layer of `deck`", which is *not* Tailwind's
> top-level `base`. Listing them in one statement would sink the entire design system
> below preflight. Hence the two statements.

### Animation

```js
import { animate, createTimeline, stagger } from "/@deck/vendor/animejs.js";

const timeline = createTimeline();
window.deck.registerTimeline(timeline); // printing seeks it to its final frame
```

`window.deck` exposes `mode`, `slideId`, `step`, `stepCount`, `reducedMotion` and
`canvas`, and dispatches these events on `document`:

| Event | When |
|---|---|
| `deck:init` | Initial state is known; `waitUntil()` is available |
| `deck:ready` | Fonts, images, components and custom promises have settled |
| `deck:enter` / `deck:leave` | The slide became / stopped being the current one |
| `deck:stepchange` | A step was applied (`from`, `to`, `direction`, `instant`) |
| `deck:pause` / `deck:resume` | The presenter paused or resumed |
| `deck:prepare-print` | Finalise state before printing; `waitUntil()` is available |
| `deck:dispose` | The iframe is about to go away |

Animation is skipped entirely in `print` and `check` mode so both are deterministic.

## Commands

```bash
deck init [DIR] --theme minimal-light     # scaffold a deck
deck add slide security --after architecture
deck dev --open presenter                 # dev server with hot reload
deck present --fullscreen                 # start presenting
deck check --changed --report sarif       # lint the deck
deck build --base-url /decks/             # static output
deck open print                           # open /print in a browser
deck component list|show|eject|new
deck doctor --json                        # diagnose the environment
```

Global options: `--config <PATH>` `--root <PATH>` `--json` `-v/--verbose` `--no-color`

Exit codes: `0` success · `1` check violations · `2` configuration or input ·
`3` browser launch or connection · `4` render or build.

`deck add slide --after` numbers the new file into the gap between its neighbours
(`20-architecture` → **`25-security`** → `30-demo`), falling back to a letter suffix when
the integers run out.

## Configuration

Merge order: built-in defaults < `deck.toml` < `deck.local.toml` < environment < CLI.

```toml
schema = 1

[deck]
title = "Chelamon Architecture"
author = "Junta Goto"
lang = "en"

[canvas]
width = 1280
height = 720
safe_area = [56, 64, 56, 64]   # top, right, bottom, left

[theme]
styles = ["design/tokens.css", "design/theme.css", "design/overrides.css"]

[tailwind]
entry = "design/tailwind.css"
preflight = true

[server]
host = "127.0.0.1"
port = 0            # 0 asks the OS for a free port
open = "presenter"
hot_reload = true
preload = 1

[animation]
engine = "animejs"
reduced_motion = "instant"

[browser]
command = "chromium"
headless = true
sandbox = true      # set false in containers and CI, where namespaces are restricted

[check]
on_save = "changed"
timeout_ms = 10_000
min_font_px = 18
external_network = "deny"

[check.rules]
slide_overflow = "error"
outside_safe_area = "warning"

[build]
output_dir = "dist"
base_url = "/"
fingerprint_assets = true
```

Environment overrides include `DECK_HOST`, `DECK_PORT`, `DECK_BROWSER`,
`DECK_BROWSER_SANDBOX`, `DECK_BASE_URL`, `DECK_HEADLESS` and `DECK_CHECK_TIMEOUT_MS`. Put machine-specific values such as a
Chromium path in `deck.local.toml`, which is not committed:

```toml
[browser]
command = "/usr/bin/chromium"
```

## Checks

```bash
deck check                                  # static + runtime + layout
deck check --static                         # no browser
deck check --slide architecture --screenshots
deck check --report json --out .deck/reports/check.json
```

| Category | Rules |
|---|---|
| Static | `duplicate_slide_id` `missing_title` `missing_deck_slide` `duplicate_html_id` `invalid_component_name` `invalid_local_url` `missing_file` `external_url` |
| Runtime | `console_error` `javascript_exception` `unhandled_rejection` `missing_asset` `missing_font` `undefined_component` `external_network` `ready_timeout` |
| Layout | `slide_overflow` `clipped_text` `outside_canvas` `outside_safe_area` `text_overlap` `min_font_size` `low_contrast` `text_density` |

Every rule has a configurable severity (`error`, `warning`, `off`) and can be suppressed:

```html
<div data-deck-check-ignore="outside-safe-area">…</div>
```

```toml
[check.ignore]
selectors = [".background-decoration"]
slides = ["appendix"]
```

Results are reproducible: viewport, device scale factor, locale, timezone and reduced
motion are all pinned, and each run uses its own browser profile.

## Static build

`deck build` produces a directory that any static HTTP server can serve, with
`/present`, `/presenter`, `/print` and every slide included. Neither Node.js nor the
deck CLI is needed to run it.

```text
dist/
├── index.html
├── present/index.html
├── presenter/index.html
├── print/index.html
├── slides/<id>/index.html
├── @deck/            # runtime, design.css, components.js, manifest.json, vendor/
├── assets/           # content-hashed filenames
└── deck-manifest.json
```

Use `--base-url /repo/` when publishing under a sub-path, such as a GitHub Pages project
site.

> Reference assets with root-absolute paths (`/assets/diagram.svg`). The static build
> relocates each slide to `slides/<id>/index.html`, so relative paths cannot resolve.
> `deck check` reports these as `missing_file` or `invalid_local_url`.

## Browser support

Chromium-based browsers only, by design. The `iframe` boundary is a mechanism for
isolating DOM, CSS and JavaScript state — **not** a security sandbox. Slide HTML is
treated as trusted local code; running untrusted decks is out of scope.

## Development

```bash
cargo test --workspace      # unit + end-to-end (browser tests skip without Chromium)
cargo clippy --workspace --all-targets
cargo fmt --all
```

```text
crates/
├── deck-cli/       # clap CLI
└── deck-core/      # config, discovery, manifest, server, watcher, browser,
                    # check, report, build, scaffold, doctor
web/                # runtime, shell, print view, design system, vendored assets
site/               # the introduction deck published to GitHub Pages
```

Everything under `web/` is embedded into the release binary, which is why normal use
needs no Node.js and no network.

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or
  <https://www.apache.org/licenses/LICENSE-2.0>)
- MIT license ([LICENSE-MIT](LICENSE-MIT) or <https://opensource.org/licenses/MIT>)

at your option.

Unless you explicitly state otherwise, any contribution intentionally submitted for
inclusion in this project by you, as defined in the Apache-2.0 license, shall be dual
licensed as above, without any additional terms or conditions.

### Bundled third-party assets

| Asset | Version | License |
|---|---|---|
| [Anime.js](https://animejs.com/) | 4.5.0 | MIT — [`web/vendor/animejs.LICENSE.md`](web/vendor/animejs.LICENSE.md) |
| [Tailwind CSS](https://tailwindcss.com/) (browser build) | 4.3.3 | MIT — [`web/vendor/tailwind.LICENSE.md`](web/vendor/tailwind.LICENSE.md) |

Both are redistributed unmodified with their license texts, and both are copied into
`deck build` output.

Rust dependencies are permissively licensed. `scraper` pulls in `cssparser`,
`cssparser-macros`, `dtoa-short` and `selectors`, which are **MPL-2.0**: file-level
copyleft that applies to those crates' own sources, not to this project's code. No
dependency is licensed under the GPL or AGPL.
