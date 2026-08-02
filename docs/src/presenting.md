# Presenting and printing

```bash
deck present              # opens /present
deck present --fullscreen
deck present --slide architecture
```

## The two views

**`/present`** is what the audience sees: the slide scaled to the window, a progress bar,
and a small HUD that fades out when the mouse stops moving.

**`/presenter`** is for you: the current slide, the next slide, speaker notes, a timer, a
clock, the slide and step number, blackout, pause, and a live diagnostics panel showing
anything the current slide reported. Open it on a second screen.

The two stay in sync over a websocket, so they work across two windows, two displays or
two devices on the same network. Whichever one you drive, the other follows — and it
follows with the same animation, so the audience view never looks like a slideshow of
static frames while yours reveals smoothly. A client that joins late is the exception: it
lands on the current slide and step directly rather than replaying the reveals it
missed.

## Controls

| | |
|---|---|
| `→` `Space` `PageDown`, click the **right half** | next step, then the next slide |
| `←` `PageUp`, right-click, click the **left half** | previous step, then the previous slide |
| `↑` `↓` | previous / next slide, skipping steps |
| `Home` `End` | first / last step |
| `b` or `.` | blackout |
| `f` | fullscreen |
| `p` or `s` | open the presenter view |
| `t` `r` *(presenter)* | start/pause and reset the timer |

The position lives in the URL — `/present#/architecture/2` — so a reload, a bookmark or a
pasted link all land in the same place.

## Only three iframes

The shell keeps a ring of three frames: previous, current, next. A hundred-slide deck
never loads a hundred iframes, and stepping back and forth is instant because the
neighbours are already warm.

Preloaded frames stay dormant: they do not run reveal animations until they are actually
shown. `[server] preload` controls how many neighbours are kept.

## Presenting from a second machine

```bash
deck present --host 0.0.0.0
```

deck prints a warning, because that makes the deck readable by anyone on the network.
The default binding is `127.0.0.1`.

## Printing

There is no PDF pipeline in the CLI. Printing is the browser's job, and `/print` is a real
page you can inspect first.

```bash
deck open print
```

The page loads every slide in print mode, drives each one to the requested step, waits for
fonts, images and any promise a slide registered, and only then enables the **Print**
button. `window.print()` is never called for you — the point is that you get to read the
preflight report before committing to paper.

| URL | Pages |
|---|---|
| `/print` or `/print?steps=final` | one page per slide, fully revealed |
| `/print?steps=initial` | one page per slide, nothing revealed |
| `/print?steps=each` | one page per step |
| `/print?slide=architecture` | one slide only |

The preflight panel lists what loaded and what each slide reported:

```text
Print preflight
✓ 18 / 18 slides loaded
⚠ architecture: [outside_safe_area] element sits outside the safe area by 9px
✗ benchmark: [clipped_text] text is cut off vertically: +12px
```

Errors do not block you. Print anyway if you know better.

In the browser's print dialog, choose **Save as PDF**, set margins to none and enable
background graphics. `@page` is already sized to the canvas, so one slide is one page. Set
`[print] show_notes = true` to print speaker notes under each slide.
