# CLI

```text
deck init        create a new deck
deck add slide   add a slide, numbered into the gap
deck dev         development server with hot reload
deck present     start presenting
deck check       lint the deck
deck build       static output
deck open        open a page in the browser
deck component   list, show, eject, scaffold components
deck doctor      diagnose the environment
```

Global options, valid on every subcommand:

```text
--config <PATH>   path to deck.toml
--root <PATH>     project root
--json            print the result as JSON
-v, --verbose     repeat for more detail
--no-color        disable coloured output
```

Exit codes:

| | |
|---|---|
| `0` | success |
| `1` | check violations |
| `2` | configuration or input error |
| `3` | browser launch or connection error |
| `4` | render or build error |

## init

```bash
deck init                          # in the current directory
deck init my-deck
deck init my-deck --title "Q1 Review" --theme dark
```

`--theme` is `default`, `minimal-light` or `dark`. Writes a working three-slide deck plus
the `deck-slides` agent skill — see [Working with agents](./agents.md).

## add slide

```bash
deck add slide architecture
deck add slide security --after architecture
```

Without `--after`, the file is appended at the next multiple of ten. With it, the number
lands in the gap: `20-architecture` → **`25-security`** → `30-demo`. If the integers run
out, a letter suffix keeps the order (`20a-…`, which still sorts between 20 and 21).

## dev

```bash
deck dev
deck dev --open present          # none | index | present | presenter | print
deck dev --slide architecture    # start on this slide
deck dev --port 5173 --host 0.0.0.0
deck dev --no-hot-reload
```

## present

```bash
deck present --fullscreen
deck present --slide architecture
```

Same server as `deck dev`, opening `/present`.

## check

```bash
deck check
deck check --static                # no browser
deck check --changed               # only what changed since last run
deck check --slide architecture --slide summary
deck check --screenshots           # a PNG per slide in .deck/screenshots/
deck check --report json|sarif|human --out FILE
```

## build

```bash
deck build
deck build --out public --base-url /decks/2026/
```

## open

```bash
deck open print        # opens /print; printing stays with the browser
deck open present
deck open presenter
```

## component

```bash
deck component list                # built-ins and yours
deck component show deck-card      # its built-in styles
deck component eject deck-card     # copy them into design/ejected/
deck component new acme-metric     # scaffold and register
```

## doctor

```bash
deck doctor
deck doctor --json
```

Checks the Chromium executable and version, the CDP connection, font availability,
writable directories, the port, and whether `deck.lock` matches the running binary. The
first thing to run when something behaves oddly.
