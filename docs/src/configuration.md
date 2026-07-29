# Configuration

`deck.toml` holds deck-wide settings and nothing else — no slide list, no slide content.

Merge order, lowest priority first:

```text
built-in defaults  <  deck.toml  <  deck.local.toml  <  environment  <  CLI arguments
```

`deck.local.toml` is for machine-specific values and is not committed. A Chromium path is
the usual reason to have one:

```toml
# deck.local.toml
[browser]
command = "/usr/bin/chromium"
sandbox = false
```

## The whole file

```toml
schema = 1

[deck]
title = "Chelamon Architecture"
author = "Junta Goto"
lang = "en"

[canvas]
width = 1280
height = 720
safe_area = [56, 64, 56, 64]     # top, right, bottom, left

[theme]
styles = ["design/tokens.css", "design/theme.css", "design/overrides.css"]

[components]
entry = "components/index.js"

[tailwind]
entry = "design/tailwind.css"
preflight = true

[server]
host = "127.0.0.1"
port = 0                         # 0 asks the OS for a free port
open = "presenter"               # none | index | present | presenter | print
hot_reload = true
preload = 1                      # neighbouring slides kept warm

[animation]
engine = "animejs"               # animejs | none
reduced_motion = "instant"       # instant | respect | ignore

[browser]
command = "chromium"
headless = true
sandbox = true

[check]
on_save = "changed"              # off | changed | all
timeout_ms = 10_000
min_font_px = 18
overflow_tolerance_px = 1
max_characters = 900
external_network = "deny"        # deny | allow

[check.rules]
slide_overflow = "error"
outside_safe_area = "warning"
# … every rule takes error | warning | off

[check.ignore]
selectors = [".background-decoration"]
slides = ["appendix"]

[print]
route = "/print"
steps = "final"                  # final | initial | each
preflight = true
show_notes = false

[build]
output_dir = "dist"
base_url = "/"                   # must start and end with '/'
fingerprint_assets = true
```

## Notes on individual settings

### canvas

`width` and `height` are the logical size every slide is laid out on; the shell scales it.
`safe_area` is the margin the `outside_safe_area` check enforces — content outside it may
be clipped by an unfamiliar projector.

### browser

`command` is resolved the way a shell would: an absolute path is used as given, a bare
name is looked up in `PATH`.

`sandbox = false` is the documented workaround for Ubuntu 23.10+, most container images
and CI runners, where unprivileged user namespaces are restricted and Chromium aborts
with "No usable sandbox". Everything deck opens is localhost.

### tailwind

`preflight` puts Tailwind's reset at the head of the entry, in `@layer base` — below the
deck design system, so it normalises the document without overriding components. Turn it
off if you would rather rely on deck's own reset alone.

### check

`on_save` reserves the behaviour for editor integrations; `deck check --changed` is the
command-line equivalent, comparing content hashes recorded under `.deck/cache/`.

`external_network = "deny"` reports any request that leaves the local origin. A deck
should not need the network while you present.

## Environment variables

Useful for CI and one-off overrides:

`DECK_TITLE` · `DECK_LANG` · `DECK_HOST` · `DECK_PORT` · `DECK_OPEN` · `DECK_PRELOAD` ·
`DECK_HOT_RELOAD` · `DECK_BROWSER` · `DECK_HEADLESS` · `DECK_BROWSER_SANDBOX` ·
`DECK_CHECK_TIMEOUT_MS` · `DECK_EXTERNAL_NETWORK` · `DECK_CANVAS_WIDTH` ·
`DECK_CANVAS_HEIGHT` · `DECK_BASE_URL` · `DECK_OUTPUT_DIR` · `DECK_FINGERPRINT_ASSETS` ·
`DECK_TAILWIND_PREFLIGHT` · `DECK_PRINT_STEPS`

`DECK_LOG` sets the tracing filter, e.g. `DECK_LOG=deck_core=debug`.

## deck.lock

Records the versions the deck was authored against: the deck runtime, Anime.js, Tailwind
CSS, the built-in components and theme. `deck build` refreshes it, `deck doctor` reports
drift. Commit it.

## Reserved URLs

Served by the dev server and emitted by the static build:

```text
/@deck/boot.js          /@deck/manifest.json     /@deck/vendor/animejs.js
/@deck/runtime.js       /@deck/design.css        /@deck/vendor/tailwind.js
/@deck/components.js    /@deck/tailwind.css      /@deck/env.js
```
