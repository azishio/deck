# The project layout

```text
my-deck/
├── .agents/skills/    # agent instructions for authoring this deck
├── .claude/skills     # symlink to .agents/skills
├── deck.toml          # deck-wide configuration
├── deck.local.toml    # machine-specific overrides (not committed)
├── deck.lock          # versions of the bundled runtime
├── slides/            # one file per slide
├── components/        # your own Custom Elements
├── design/            # tokens.css / theme.css / overrides.css / tailwind.css
├── assets/            # images/ icons/ fonts/ data/ (+ cursor.svg)
├── dist/              # output of `deck build`
└── .deck/             # cache, reports, screenshots
```

The names `slides/`, `components/`, `design/` and `assets/` are **fixed by convention**
and cannot be reconfigured. A predictable layout is what lets the CLI, the file watcher,
the static build and any agent editing the deck all stay simple.

## What each directory is for

**`slides/`** — one complete HTML document per slide. Order comes from the file path,
sorted lexicographically. Numbers go up in tens so there is room to insert:

```text
slides/
├── 00-title.html
├── 10-background.html
├── 20-architecture/
│   ├── 00-overview.html
│   └── 10-ingestion.html
└── 90-summary.html
```

Subdirectories are walked recursively and sort as part of the path, so
`20-architecture/00-overview.html` comes after `10-background.html`. Files starting with
`_` or `.` are skipped, which is handy for drafts.

**`components/`** — your Custom Elements. `components/index.js` is the entry point;
everything it imports is available in every slide. See
[Adding your own components](./own-components.md).

**`design/`** — the deck's look, in four files that differ only in cascade order. See
[Styling and theming](./styling.md).

**`assets/`** — anything served at `/assets/…`. Fonts placed in `assets/fonts/` register
themselves. See [Assets, fonts and the cursor](./assets.md).

**`.deck/`** — generated: the `--changed` cache, reports and screenshots. Safe to delete.

**`dist/`** — the static build. Safe to delete.

## Order, identity and title

Three different things, deliberately kept separate:

| | Comes from | Changes when |
|---|---|---|
| Order | the file path | you rename or renumber |
| Identity | `<deck-slide id="…">` | you decide to change it |
| Title | `<title>` | you edit the heading |

Because identity is independent of order, renumbering a deck never breaks a deep link,
a presenter-view sync or a hot reload's idea of where you are. If you omit the `id`, it
falls back to the relative path without the extension, which works but ties the two
together — write an explicit `id` on any slide you might link to.
