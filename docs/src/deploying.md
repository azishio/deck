# Static build and deployment

```bash
deck build
```

`dist/` is a plain folder any static HTTP server can serve. Neither Node.js nor the deck
CLI is needed to run it.

```text
dist/
├── index.html                # the slide index
├── present/index.html
├── presenter/index.html
├── print/index.html
├── slides/<id>/index.html    # one directory per stable id
├── @deck/                    # runtime, design.css, components.js, manifest.json, vendor/
├── assets/                   # content-hashed file names
└── deck-manifest.json
```

Hot reload and the presenter/audience websocket sync are gone in a static build, as you
would expect — the shell notices the missing server and carries on.

## Serving under a sub-path

```bash
deck build --base-url /decks/2026-architecture/
```

Every root-absolute URL is rewritten onto that base exactly once, including the runtime
tags and the fingerprinted asset names.

<div class="warning">

Reference assets with root-absolute paths (`/assets/diagram.svg`). The build relocates
each slide to `slides/<id>/index.html`, so relative paths cannot resolve. `deck check`
catches these before you deploy.

</div>

## Fingerprinting

`[build] fingerprint_assets` (on by default) renames assets with a content hash —
`logo.a1b2c3d4.png` — and rewrites the references. Set a far-future cache header and
never think about cache invalidation again.

## GitHub Pages

This guide and the [introduction deck](/deck/slide/present) are both published this way,
from one workflow. The book goes to the root and the deck to a sub-path, so they never
fight over URLs:

```yaml
- uses: actions/configure-pages@v6
  id: pages

- name: Build the deck
  run: |
    deck build --base-url "${{ steps.pages.outputs.base_path }}/slide/" \
               --out _site/slide

- name: Build the guide
  run: mdbook build docs --dest-dir ../_site

- uses: actions/upload-pages-artifact@v5
  with:
    path: _site
```

`configure-pages` reports the base path (`/deck` for a project site, empty for a user
site), which is exactly what `--base-url` needs.

The full workflow is in
[`.github/workflows/pages.yml`](https://github.com/azishio/deck/blob/main/.github/workflows/pages.yml).

## Anywhere else

`dist/` has no server requirements beyond serving files and directory indexes, so
Netlify, Cloudflare Pages, S3 with CloudFront, `nginx`, or `python3 -m http.server` all
work unchanged. For a one-off share, zip it — opening `index.html` from the file system
will not work, because ES modules and `fetch` need an origin, but any local server does.
