# deck

**One slide is one complete HTML document.**

`deck` is a local slide runtime and development environment written in Rust. A slide is
a file you can read, diff and review — plain HTML, CSS and JavaScript. There is no slide
DSL, no required Markdown, and no slide body hidden inside TOML or JSON.

<div class="warning">

New here? Read [Install and create a deck](./getting-started.md), then
[Writing a slide](./writing-slides.md). Everything else is reference material you can
come back to.

</div>

## See it running

The [introduction deck](/deck/slide/present) is itself a deck, built by `deck build` and
published next to this guide. Its source lives in [`site/`](https://github.com/azishio/deck/tree/main/site)
if you want to read a real deck rather than snippets.

## What you get

| | |
|---|---|
| **Directory-based discovery** | `slides/**/*.html` in lexicographic order. No slide list to maintain. |
| **iframe isolation** | Per-slide JavaScript, CSS and DOM state, with a three-frame ring so 100 slides do not mean 100 iframes. |
| **Absolute steps** | `screen state = f(slide_id, step)`. Deep links and reloads land exactly where you left off. |
| **Tailwind CSS** | v4 browser build, vendored. Utilities compile inside each slide — no bundler, no CDN. |
| **Anime.js** | Vendored and importable straight from a slide. |
| **Hot reload** | HTML, CSS and component edits apply while keeping the current slide, step and presenter timer. |
| **Presenter View** | Current and next slide, speaker notes, timer, clock, blackout, live diagnostics. |
| **Printing** | A real `/print` page for the browser's own print dialog. The CLI never generates a PDF. |
| **Checks** | headless Chromium finds layout, runtime and asset problems. Human, JSON or SARIF output. |
| **Static build** | `deck build` emits a folder any static host can serve. |

## Why plain HTML

Most slide tools ask you to learn their format. This one asks you to use the web
platform you already know.

- **Editable by humans and agents alike.** No intermediate representation to translate
  through, so "add a slide about X" is a request an agent can carry out and you can
  review as a diff.
- **Nothing is off-limits.** SVG, Canvas, WebGL, a live iframe, a custom element you
  wrote this morning — if it runs in Chromium, it runs in a slide.
- **Isolated by construction.** One slide's JavaScript cannot leak into the next, so a
  broken experiment on slide 12 does not take down slide 13.
- **Verifiable.** Overflow, clipped text, broken assets and console errors are lint
  rules, not things you discover from the back of the room.

## What it is not

`deck` targets **Chromium-based browsers only**, on purpose. The `iframe` boundary
isolates DOM, CSS and JavaScript state; it is **not** a security sandbox, and slide HTML
is treated as trusted local code. Running untrusted decks is out of scope.

The CLI never produces a PDF. Printing belongs to the browser, and `/print` is a real
page you can inspect before committing to paper.
