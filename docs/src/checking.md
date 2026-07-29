# Checking

Slides fail in ways that are invisible until someone is in the room: a line of text one
pixel too tall for its box, an image that 404s, a console error that stops the animation
on slide 14. `deck check` finds them with headless Chromium.

```bash
deck check                                   # static + runtime + layout
deck check --static                          # no browser, fast
deck check --changed                         # only what you edited
deck check --slide architecture --screenshots
deck check --report json --out .deck/reports/check.json
```

Exit code `1` means violations, so it drops into CI as-is.

## What it checks

**Static** — no browser needed:

`duplicate_slide_id` · `missing_title` · `missing_deck_slide` · `duplicate_html_id` ·
`invalid_component_name` · `invalid_local_url` · `missing_file` · `external_url`

**Runtime** — with the slide actually running:

`console_error` · `javascript_exception` · `unhandled_rejection` · `missing_asset` ·
`missing_font` · `undefined_component` · `external_network` · `ready_timeout` ·
`step_count_mismatch`

**Layout** — measured in the browser at the real canvas size:

`slide_overflow` · `clipped_text` · `outside_canvas` · `outside_safe_area` ·
`text_overlap` · `min_font_size` · `low_contrast` · `text_density`

## Reading a failure

```text
security  30-security.html
  error   [clipped_text] text is cut off vertically: +12px
          #security > deck-grid > deck-card:nth-of-type(2)
          rect 514x188 @ (677, 232)
  warning [low_contrast] contrast ratio is too low: 4.36 < 4.5
```

Each finding carries the slide, the source file, a CSS selector and the element's
bounding box, so you can go straight to it. With `--screenshots` the report also points
at a PNG per slide under `.deck/screenshots/`.

What they usually mean:

| Rule | Usually means |
|---|---|
| `slide_overflow` | too much content; cut text or split the slide |
| `clipped_text` | a fixed-height box is smaller than its text |
| `outside_canvas` | an element escapes the 1280×720 canvas entirely |
| `outside_safe_area` | content is closer to an edge than the safe area allows |
| `min_font_size` | below 18px; unreadable from the back of a room |
| `low_contrast` | below WCAG AA against the actual computed background |
| `text_overlap` | two text blocks sit on top of each other |
| `missing_file` | a referenced asset does not exist, or the path is relative |
| `undefined_component` | a tag whose Custom Element never registered |

## Severity and suppression

Every rule is `error`, `warning` or `off`:

```toml
[check.rules]
outside_safe_area = "warning"
text_density = "off"
```

Suppress a single element, naming the rule:

```html
<div data-deck-check-ignore="outside-safe-area">…</div>
<div data-deck-check-ignore="*">…</div>
```

Or by selector, deck-wide:

```toml
[check.ignore]
selectors = [".background-decoration", "[data-intentional-overflow]"]
slides = ["appendix"]
```

Prefer fixing over suppressing. A decorative shape that intentionally bleeds off the
canvas is a fair suppression; a heading that does not fit is not.

## Reproducibility

Results are stable run to run: the viewport, device scale factor, locale, timezone and
reduced-motion preference are pinned, animation is disabled in check mode, and each run
gets its own browser profile. Two runs on the same deck produce the same report, which is
what makes it useful in CI.

## In CI

```yaml
- run: deck check --report sarif --out check.sarif
  env:
    DECK_BROWSER: chrome
    DECK_BROWSER_SANDBOX: "false"   # runners restrict user namespaces
```

SARIF uploads as code-scanning results, which annotates the pull request line by line.
`--report json` is easier to post-process yourself.
