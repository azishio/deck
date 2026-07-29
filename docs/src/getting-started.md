# Install and create a deck

## Requirements

- A Rust toolchain, to build the CLI.
- A Chromium-based browser — Chrome, Chromium, Edge or Brave.

**Node.js is not required.** Tailwind CSS and Anime.js are vendored into the binary, so
normal use touches neither npm nor a CDN.

## Install

```bash
cargo install --git https://github.com/azishio/deck --locked deck-cli
```

Or from a clone, which is also what you want for hacking on deck itself:

```bash
git clone https://github.com/azishio/deck
cd deck
cargo install --path crates/deck-cli --locked
```

Check that the environment is sound before you start authoring:

```bash
deck doctor
```

It reports the Chromium executable and version, the CDP connection, available fonts,
writable directories, the port and whether `deck.lock` matches the running binary. If
Chromium lives somewhere unusual, or its sandbox is unavailable (containers and most CI
runners), see [Configuration](./configuration.md#browser).

## Create a deck

```bash
deck init my-deck
cd my-deck
deck dev
```

`deck dev` starts a local server, opens the presenter view and watches your files.
Editing a slide updates the screen without losing your place.

The generated project is a working three-slide deck, not an empty shell — see
[Editing the template](./editing-the-template.md) for a tour of what to change first.

Pick a starting look with `--theme`:

```bash
deck init my-deck --theme minimal-light   # default | minimal-light | dark
```

## The four commands you will actually use

```bash
deck dev                # author, with hot reload
deck check              # lint the deck before you present
deck present            # present it
deck build              # hand it out as a static site
```

Everything else is in the [CLI reference](./cli.md).
