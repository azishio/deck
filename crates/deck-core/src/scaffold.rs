//! Project scaffolding: `deck init`, `deck add slide`, `deck component`.

use camino::{Utf8Path, Utf8PathBuf};

use crate::assets;
use crate::config::{CONFIG_FILE, Config};
use crate::discovery;
use crate::error::{Error, Result, read_to_string, write_file};
use crate::lock::Lock;
use crate::project::Project;

/* -------------------------------------------------------------------------- */
/* deck init                                                                   */
/* -------------------------------------------------------------------------- */

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Theme {
    #[default]
    Default,
    MinimalLight,
    Dark,
}

impl Theme {
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "default" => Some(Self::Default),
            "minimal-light" => Some(Self::MinimalLight),
            "dark" => Some(Self::Dark),
            _ => None,
        }
    }

    pub const ALL: [&'static str; 3] = ["default", "minimal-light", "dark"];

    fn theme_css(self) -> &'static str {
        match self {
            Self::Default => {
                "/* Project theme. Loaded into @layer project. */\n:root {\n  --deck-color-accent: #c62828;\n}\n"
            }
            Self::MinimalLight => {
                "/* minimal-light */\n:root {\n  --deck-color-background: #ffffff;\n  --deck-color-surface: #ffffff;\n  --deck-color-border: #e4e7ec;\n  --deck-color-text: #101828;\n  --deck-color-accent: #1d4ed8;\n  --deck-shadow-card: none;\n}\n\ndeck-card {\n  box-shadow: none;\n}\n"
            }
            Self::Dark => {
                "/* dark */\n:root {\n  --deck-color-background: #0f1720;\n  --deck-color-surface: #16202b;\n  --deck-color-surface-strong: #1d2937;\n  --deck-color-border: #26323f;\n  --deck-color-text: #e6edf3;\n  --deck-color-muted: #98a2b3;\n  --deck-color-accent: #60a5fa;\n  --deck-color-accent-soft: #17273c;\n}\n"
            }
        }
    }
}

/// Create a new deck project. Fails if the directory already contains one.
pub fn init(root: &Utf8Path, title: &str, theme: Theme) -> Result<()> {
    if root.join(CONFIG_FILE).exists() {
        return Err(Error::config(format!("{} is already a deck project", root)));
    }

    write_file(&root.join(CONFIG_FILE), deck_toml(title))?;
    write_file(&root.join("design/tokens.css"), TOKENS_TEMPLATE)?;
    write_file(&root.join("design/theme.css"), theme.theme_css())?;
    write_file(&root.join("design/overrides.css"), OVERRIDES_TEMPLATE)?;
    write_file(&root.join("design/tailwind.css"), TAILWIND_TEMPLATE)?;
    write_file(&root.join("components/index.js"), COMPONENTS_INDEX_TEMPLATE)?;
    write_file(&root.join("components/example-badge.js"), EXAMPLE_COMPONENT_TEMPLATE)?;
    for subdir in assets::ASSET_SUBDIRS {
        write_file(&root.join("assets").join(subdir).join(".gitkeep"), "")?;
    }
    write_file(&root.join("assets/README.md"), ASSETS_README)?;
    write_file(&root.join(".gitignore"), GITIGNORE_TEMPLATE)?;
    install_skills(root)?;

    write_file(&root.join("slides/00-title.html"), title_slide(title))?;
    write_file(&root.join("slides/10-overview.html"), OVERVIEW_SLIDE)?;
    write_file(&root.join("slides/20-architecture.html"), ARCHITECTURE_SLIDE)?;

    Lock::default().save(root)?;
    Ok(())
}

/// Agent instructions for working on the generated deck, split by task so the
/// right one loads for the job at hand.
///
/// The canonical copies live in `.agents/skills/`, with `.claude/skills` linked
/// to them so Claude Code picks them up without a second copy to keep in sync.
pub const SKILLS: &[(&str, &str)] = &[
    ("deck-slides", include_str!("../templates/skills/deck-slides.md")),
    ("deck-styling", include_str!("../templates/skills/deck-styling.md")),
    ("deck-components", include_str!("../templates/skills/deck-components.md")),
];
const SKILL_DIR: &str = ".agents/skills";

fn install_skills(root: &Utf8Path) -> Result<()> {
    for (name, body) in SKILLS {
        write_file(&root.join(SKILL_DIR).join(name).join("SKILL.md"), body)?;
    }

    let link = root.join(".claude").join("skills");
    if link.exists() || link.symlink_metadata().is_ok() {
        return Ok(());
    }
    std::fs::create_dir_all(link.parent().expect("has a parent"))
        .map_err(|error| Error::io(&link, error))?;

    match symlink_dir(Utf8Path::new("..").join(SKILL_DIR).as_std_path(), link.as_std_path()) {
        Ok(()) => Ok(()),
        Err(error) => {
            // Windows needs developer mode or elevation for symlinks; copies
            // keep the skills discoverable, at the cost of duplicate files.
            tracing::warn!("could not symlink {link} ({error}); copying instead");
            for (name, body) in SKILLS {
                write_file(&link.join(name).join("SKILL.md"), body)?;
            }
            Ok(())
        }
    }
}

#[cfg(unix)]
fn symlink_dir(target: &std::path::Path, link: &std::path::Path) -> std::io::Result<()> {
    std::os::unix::fs::symlink(target, link)
}

#[cfg(windows)]
fn symlink_dir(target: &std::path::Path, link: &std::path::Path) -> std::io::Result<()> {
    std::os::windows::fs::symlink_dir(target, link)
}

#[cfg(not(any(unix, windows)))]
fn symlink_dir(_target: &std::path::Path, _link: &std::path::Path) -> std::io::Result<()> {
    Err(std::io::Error::other("symlinks are unsupported on this platform"))
}

fn deck_toml(title: &str) -> String {
    let default = Config::default();
    format!(
        r#"schema = 1

[deck]
title = "{title}"
author = ""
lang = "en"

[canvas]
width = {width}
height = {height}
safe_area = [56, 64, 56, 64]

[theme]
styles = [
  "design/tokens.css",
  "design/theme.css",
  "design/overrides.css",
]

[components]
entry = "components/index.js"

[tailwind]
entry = "design/tailwind.css"
preflight = true

[server]
host = "127.0.0.1"
port = 0
open = "presenter"
hot_reload = true
preload = 1

[animation]
engine = "animejs"
reduced_motion = "instant"

[browser]
command = "chromium"
headless = true
sandbox = true

[check]
on_save = "changed"
timeout_ms = 10000
min_font_px = 18
overflow_tolerance_px = 1
external_network = "deny"

[check.rules]
console_error = "error"
unhandled_rejection = "error"
missing_asset = "error"
missing_font = "error"
undefined_component = "error"
slide_overflow = "error"
clipped_text = "error"
outside_canvas = "error"
outside_safe_area = "warning"
text_overlap = "warning"

[print]
route = "/print"
steps = "final"
preflight = true
show_notes = false

[build]
output_dir = "dist"
base_url = "/"
fingerprint_assets = true
"#,
        width = default.canvas.width,
        height = default.canvas.height,
    )
}

const TOKENS_TEMPLATE: &str = "/* Project tokens. Override any --deck-* custom property here. */\n:root {\n  /* --deck-font-sans: \"Meiryo\", sans-serif; */\n}\n";

const OVERRIDES_TEMPLATE: &str = "/* Last-word overrides. Loaded after theme.css. */\n";

const TAILWIND_TEMPLATE: &str = r#"/* Tailwind entry. Compiled inside every slide by the vendored browser build.
   deck prepends the reset (tailwindcss/preflight.css), then the theme and the
   utilities, and maps the deck tokens onto the Tailwind theme. Only project
   additions go here: extra @theme values, @utility definitions and @apply. */

@theme {
  /* --color-brand: oklch(0.6 0.18 25); */
}

/* @utility slide-lead {
  font-size: var(--deck-font-size-body);
  color: var(--deck-color-muted);
} */
"#;

const COMPONENTS_INDEX_TEMPLATE: &str = "// Entry point for project components. Imported by /@deck/components.js.\nimport \"./example-badge.js\";\n";

const EXAMPLE_COMPONENT_TEMPLATE: &str = r#"// Light DOM component example. Use a project-specific prefix; `deck-*` is reserved.
class ExampleBadge extends HTMLElement {
  connectedCallback() {
    this.style.display = "inline-block";
    this.style.padding = "4px 12px";
    this.style.borderRadius = "999px";
    this.style.background = "var(--deck-color-accent-soft)";
    this.style.color = "var(--deck-color-accent)";
    this.style.fontSize = "var(--deck-font-size-small)";
    this.style.fontWeight = "700";
  }
}

customElements.define("example-badge", ExampleBadge);
"#;

const GITIGNORE_TEMPLATE: &str = "/dist/\n/.deck/\n/deck.local.toml\n";

const ASSETS_README: &str = r#"# assets/

Everything here is served from `/assets/…`, so reference it with a
root-absolute path: `<img src="/assets/images/diagram.svg">`. Relative paths
break in `deck build`, which relocates each slide to `slides/<id>/index.html`.

| Directory | For |
|---|---|
| `images/` | photographs, diagrams, screenshots |
| `icons/`  | small SVGs used inline or as backgrounds |
| `fonts/`  | webfonts — see below |
| `data/`   | JSON, CSV and anything a slide fetches at runtime |

## fonts/

Font files are registered automatically; no `@font-face` to write. The file
name carries the metadata:

```text
Inter.woff2                  -> Inter, weight 400, normal
Inter-700.woff2              -> Inter, weight 700
Inter-SemiBold.woff2         -> Inter, weight 600
NotoSansJP-Bold-Italic.woff2 -> NotoSansJP, weight 700, italic
```

`.woff2`, `.woff`, `.ttf` and `.otf` are recognised. Point a token at the
family and every slide picks it up:

```css
/* design/tokens.css */
:root {
  --deck-font-sans: "Inter", sans-serif;
}
```

`deck check` reports a `missing_font` error if a family is referenced but never
loads.

## Custom cursor

Drop `cursor.svg`, `cursor.png`, `cursor.webp`, `cursor.gif` or `cursor.jpg`
directly in `assets/` and it replaces the mouse cursor on every slide and in
the presentation view. SVG cursors need an explicit `width` and `height`, and
browsers ignore images larger than 128x128.
"#;

fn title_slide(title: &str) -> String {
    format!(
        r#"<!doctype html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <title>{title}</title>

  <link rel="stylesheet" href="/@deck/design.css">
  <script type="module" src="/@deck/boot.js"></script>
</head>
<body>
  <deck-slide id="title" layout="title" class="bg-linear-to-br from-surface to-background">
    <deck-heading level="title" eyebrow="Deck" sub="One slide is one complete HTML document.">
      {title}
    </deck-heading>

    <p class="text-muted text-small">Tailwind CSS utilities work out of the box.</p>

    <deck-notes>
      The opening slide. Press the right arrow, or click the right half, to advance.
    </deck-notes>
  </deck-slide>
</body>
</html>
"#
    )
}

const OVERVIEW_SLIDE: &str = r#"<!doctype html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <title>Overview</title>

  <link rel="stylesheet" href="/@deck/design.css">
  <script type="module" src="/@deck/boot.js"></script>
</head>
<body>
  <deck-slide id="overview">
    <deck-heading eyebrow="Overview">
      Written in plain HTML, CSS and JavaScript
    </deck-heading>

    <deck-stack gap="24">
      <p data-step="1" class="flex items-center gap-3">
        <span class="inline-flex size-8 shrink-0 items-center justify-center rounded-full bg-accent text-white text-small font-bold">1</span>
        Every slide is a complete HTML document of its own.
      </p>
      <p data-step="2" class="flex items-center gap-3">
        <span class="inline-flex size-8 shrink-0 items-center justify-center rounded-full bg-accent text-white text-small font-bold">2</span>
        The design system is Web Components, CSS Custom Properties and Tailwind CSS.
      </p>
      <p data-step="3" class="flex items-center gap-3">
        <span class="inline-flex size-8 shrink-0 items-center justify-center rounded-full bg-accent text-white text-small font-bold">3</span>
        Anime.js ships with the runtime; import it and animate.
      </p>
    </deck-stack>

    <deck-notes>
      data-step reveals content in stages. Steps are absolute, never relative.
    </deck-notes>
  </deck-slide>
</body>
</html>
"#;

const ARCHITECTURE_SLIDE: &str = r#"<!doctype html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
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
      <deck-card data-step="1" class="justify-between">
        <h2 class="text-body font-bold">Collection</h2>
        <p>Probes push measurements into the ingest tier.</p>
        <span class="text-small text-muted font-mono">probe -&gt; ingest</span>
      </deck-card>

      <deck-card data-step="2" variant="accent" class="justify-between">
        <h2 class="text-body font-bold">Visualization</h2>
        <p>Queries fan out to the visualisation layer.</p>
        <span class="text-small text-accent font-mono">query -&gt; render</span>
      </deck-card>
    </deck-grid>

    <deck-callout tone="info" label="Note" data-step="3">
      Richer animation is just JavaScript inside the slide.
    </deck-callout>

    <deck-notes>
      Explain the ingest path and the query path separately.
    </deck-notes>
  </deck-slide>

  <script type="module">
    document.addEventListener("deck:stepchange", (event) => {
      if (event.detail.to === 3) {
        document.querySelector("deck-callout")?.setAttribute("tone", "success");
      }
    });
  </script>
</body>
</html>
"#;

/* -------------------------------------------------------------------------- */
/* deck add slide                                                              */
/* -------------------------------------------------------------------------- */

/// Create a new slide, numbering it into the gap after `after` when given.
pub fn add_slide(project: &Project, name: &str, after: Option<&str>) -> Result<Utf8PathBuf> {
    let slides_dir = project.slides_dir();
    let existing = discovery::slide_files(&slides_dir)?;

    let (directory, neighbours) = match after {
        Some(anchor) => {
            let anchor_path = resolve_slide_path(project, &existing, anchor)?;
            let directory = Utf8PathBuf::from(&anchor_path)
                .parent()
                .map(Utf8Path::to_path_buf)
                .unwrap_or_default();
            (directory.clone(), siblings(&existing, &directory))
        }
        None => (Utf8PathBuf::new(), siblings(&existing, Utf8Path::new(""))),
    };

    let prefix = match after {
        Some(anchor) => {
            let anchor_path = resolve_slide_path(project, &existing, anchor)?;
            let anchor_name = file_name(&anchor_path);
            next_prefix_after(&neighbours, anchor_name)?
        }
        None => next_prefix_at_end(&neighbours),
    };

    let file_name = format!("{prefix}-{name}.html");
    let relative = if directory.as_str().is_empty() {
        Utf8PathBuf::from(&file_name)
    } else {
        directory.join(&file_name)
    };
    let path = slides_dir.join(&relative);
    if path.exists() {
        return Err(Error::config(format!("{path} already exists")));
    }

    write_file(&path, new_slide(name))?;
    Ok(path)
}

fn resolve_slide_path(project: &Project, existing: &[String], anchor: &str) -> Result<String> {
    let slides_dir = project.slides_dir();
    if let Some(found) = existing.iter().find(|path| discovery::id_from_path(path) == anchor) {
        return Ok(found.clone());
    }
    for candidate in existing {
        let document =
            crate::manifest::SlideDocument::parse(&slides_dir.join(candidate), candidate)?;
        if document.id == anchor {
            return Ok(candidate.clone());
        }
    }
    Err(Error::config(format!("--after refers to a slide that does not exist: {anchor}")))
}

fn siblings(existing: &[String], directory: &Utf8Path) -> Vec<String> {
    existing
        .iter()
        .filter(|path| {
            let parent = Utf8Path::new(path).parent().unwrap_or(Utf8Path::new(""));
            parent == directory
        })
        .map(|path| file_name(path).to_owned())
        .collect()
}

fn file_name(path: &str) -> &str {
    path.rsplit('/').next().unwrap_or(path)
}

fn numeric_prefix(file_name: &str) -> Option<u32> {
    let digits: String = file_name.chars().take_while(char::is_ascii_digit).collect();
    digits.parse().ok()
}

fn next_prefix_at_end(siblings: &[String]) -> String {
    let highest = siblings.iter().filter_map(|name| numeric_prefix(name)).max().unwrap_or(0);
    format!("{:02}", highest.saturating_add(10).min(99))
}

/// Use the gap between the anchor and the next slide (20 → 25 → 30).
fn next_prefix_after(siblings: &[String], anchor: &str) -> Result<String> {
    let mut numbered: Vec<(u32, &str)> =
        siblings.iter().filter_map(|name| Some((numeric_prefix(name)?, name.as_str()))).collect();
    numbered.sort();

    let anchor_number = numeric_prefix(anchor)
        .ok_or_else(|| Error::config(format!("file name is not numbered: {anchor}")))?;
    let next_number =
        numbered.iter().map(|(number, _)| *number).find(|number| *number > anchor_number);

    match next_number {
        Some(next) if next - anchor_number >= 2 => {
            Ok(format!("{:02}", anchor_number + (next - anchor_number) / 2))
        }
        // No integer gap: a letter suffix keeps lexicographic order (20 < 20a < 21).
        Some(_) => {
            for suffix in 'a'..='z' {
                let candidate = format!("{anchor_number:02}{suffix}");
                if !siblings.iter().any(|name| name.starts_with(&candidate)) {
                    return Ok(candidate);
                }
            }
            Err(Error::config("no free number left; rename the surrounding slides"))
        }
        None => Ok(format!("{:02}", anchor_number.saturating_add(10).min(99))),
    }
}

fn new_slide(name: &str) -> String {
    format!(
        r#"<!doctype html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <title>{name}</title>

  <link rel="stylesheet" href="/@deck/design.css">
  <script type="module" src="/@deck/boot.js"></script>
</head>
<body>
  <deck-slide id="{name}">
    <deck-heading eyebrow="{name}">
      Write a heading
    </deck-heading>

    <deck-stack gap="16">
      <p data-step="1" class="text-muted">Write the body. Tailwind utilities are available too.</p>
    </deck-stack>

    <deck-notes>
      Notes for what you plan to say.
    </deck-notes>
  </deck-slide>
</body>
</html>
"#
    )
}

/* -------------------------------------------------------------------------- */
/* deck component                                                              */
/* -------------------------------------------------------------------------- */

#[derive(Debug, Clone)]
pub struct ComponentInfo {
    pub name: String,
    pub built_in: bool,
    pub source: Option<String>,
}

pub fn list_components(project: &Project) -> Vec<ComponentInfo> {
    let mut components: Vec<ComponentInfo> = assets::BUILT_IN_COMPONENT_NAMES
        .iter()
        .map(|name| ComponentInfo { name: (*name).to_owned(), built_in: true, source: None })
        .collect();

    let dir = project.components_dir();
    if dir.is_dir() {
        for entry in walkdir::WalkDir::new(&dir).into_iter().flatten() {
            let Some(path) = Utf8Path::from_path(entry.path()) else { continue };
            if !path.extension().is_some_and(|extension| matches!(extension, "js" | "mjs")) {
                continue;
            }
            let relative = project.relative(path).unwrap_or_else(|| path.to_string());
            for tag in crate::watcher::custom_element_tags(path) {
                components.push(ComponentInfo {
                    name: tag,
                    built_in: false,
                    source: Some(relative.clone()),
                });
            }
        }
    }
    components
}

/// CSS rules whose selector mentions `tag`, taken from the built-in stylesheet.
pub fn component_css(tag: &str) -> String {
    const COMPONENTS_CSS: &str = include_str!("../../../web/design-system/components.css");
    let mut out = String::new();
    collect_rules_for(COMPONENTS_CSS, tag, &mut out);
    out
}

/// Walk a stylesheet, descending into at-rule blocks such as `@layer`.
fn collect_rules_for(css: &str, tag: &str, out: &mut String) {
    let mut rest = css;
    while let Some(open) = rest.find('{') {
        let selector = strip_comments(&rest[..open]);
        let selector = selector.trim();
        let Some(close) = matching_brace(&rest[open..]) else { break };
        let block = &rest[open..=open + close];

        if selector.starts_with('@') {
            collect_rules_for(&block[1..block.len() - 1], tag, out);
        } else if selector
            .split(',')
            .any(|part| part.trim().split([' ', '>', ':', '[']).next() == Some(tag))
        {
            out.push_str(selector);
            out.push(' ');
            out.push_str(block);
            out.push_str("\n\n");
        }
        rest = &rest[open + close + 1..];
    }
}

/// Drop `/* … */` so a section banner is not mistaken for part of a selector.
fn strip_comments(css: &str) -> String {
    let mut out = String::with_capacity(css.len());
    let mut rest = css;
    while let Some(start) = rest.find("/*") {
        out.push_str(&rest[..start]);
        match rest[start + 2..].find("*/") {
            Some(end) => rest = &rest[start + 2 + end + 2..],
            None => return out,
        }
    }
    out.push_str(rest);
    out
}

fn matching_brace(source: &str) -> Option<usize> {
    let mut depth = 0usize;
    for (index, byte) in source.char_indices() {
        match byte {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    return Some(index);
                }
            }
            _ => {}
        }
    }
    None
}

/// Copy a built-in component's styles into the project so they can be edited.
pub fn eject_component(project: &Project, tag: &str) -> Result<Utf8PathBuf> {
    if !assets::BUILT_IN_COMPONENT_NAMES.contains(&tag) {
        return Err(Error::config(format!("not a built-in component: {tag}")));
    }
    let css = component_css(tag);
    if css.is_empty() {
        return Err(Error::config(format!("no style definition found for {tag}")));
    }

    let path = project.design_dir().join("ejected").join(format!("{tag}.css"));
    write_file(
        &path,
        format!(
            "/* Ejected from deck's built-in design system.\n   Add it to [theme].styles in deck.toml to take effect. */\n@layer project {{\n{css}}}\n"
        ),
    )?;
    Ok(path)
}

/// Scaffold a new project component and register it in the entry point.
pub fn new_component(project: &Project, tag: &str) -> Result<Utf8PathBuf> {
    if !tag.contains('-') {
        return Err(Error::config(format!("a Custom Element name needs a hyphen: {tag}")));
    }
    if tag.starts_with("deck-") {
        return Err(Error::config("deck-* is reserved for the built-in components"));
    }

    let path = project.components_dir().join(format!("{tag}.js"));
    if path.exists() {
        return Err(Error::config(format!("{path} already exists")));
    }
    write_file(&path, component_template(tag))?;

    let entry = project.components_entry();
    let import = format!("import \"./{tag}.js\";\n");
    let current = if entry.is_file() { read_to_string(&entry)? } else { String::new() };
    if !current.contains(&import) {
        write_file(&entry, format!("{current}{import}"))?;
    }
    Ok(path)
}

fn component_template(tag: &str) -> String {
    let class = tag
        .split(['-', '_'])
        .map(|part| {
            let mut chars = part.chars();
            chars
                .next()
                .map(|first| first.to_uppercase().to_string() + chars.as_str())
                .unwrap_or_default()
        })
        .collect::<String>();

    format!(
        r#"// {tag}: Light DOM component. Style it with normal CSS in design/.
class {class} extends HTMLElement {{
  static observedAttributes = ["label"];

  connectedCallback() {{
    this.#render();
  }}

  attributeChangedCallback() {{
    if (this.isConnected) {{
      this.#render();
    }}
  }}

  #render() {{
    // Keep generated nodes idempotent: Light DOM children belong to the author.
    let label = this.querySelector(":scope > .{tag}__label");
    const text = this.getAttribute("label");
    if (!text) {{
      label?.remove();
      return;
    }}
    if (!label) {{
      label = document.createElement("span");
      label.className = "{tag}__label";
      this.prepend(label);
    }}
    label.textContent = text;
  }}
}}

customElements.define("{tag}", {class});
"#
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numbers_new_slides_into_gaps() {
        let siblings = [
            "20-architecture.html".to_owned(),
            "30-demo.html".to_owned(),
            "90-summary.html".to_owned(),
        ];
        assert_eq!(next_prefix_after(&siblings, "20-architecture.html").unwrap(), "25");
        assert_eq!(next_prefix_at_end(&siblings), "99");
    }

    #[test]
    fn falls_back_to_a_letter_suffix_when_full() {
        let siblings = ["20-a.html".to_owned(), "21-b.html".to_owned()];
        assert_eq!(next_prefix_after(&siblings, "20-a.html").unwrap(), "20a");
    }

    #[test]
    fn every_skill_has_usable_frontmatter() {
        for (name, body) in SKILLS {
            let mut lines = body.lines();
            assert_eq!(lines.next(), Some("---"), "{name}: SKILL.md must open with frontmatter");
            let frontmatter: Vec<&str> = lines.take_while(|line| *line != "---").collect();
            assert!(
                frontmatter.contains(&format!("name: {name}").as_str()),
                "{name}: frontmatter name must match the directory"
            );
            assert!(
                frontmatter.iter().any(|line| line.starts_with("description: ")),
                "{name}: needs a description, which is what decides when it loads"
            );
        }
    }

    #[test]
    fn init_links_claude_skills_at_the_canonical_copies() {
        let root = Utf8PathBuf::from_path_buf(std::env::temp_dir())
            .expect("temp dir is UTF-8")
            .join(format!("deck-skill-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        init(&root, "T", Theme::Default).expect("init");

        for (name, _) in SKILLS {
            let canonical = root.join(".agents/skills").join(name).join("SKILL.md");
            let linked = root.join(".claude/skills").join(name).join("SKILL.md");
            assert!(canonical.is_file(), "{name}: missing canonical copy");
            assert_eq!(
                std::fs::read_to_string(&linked).expect("readable through the link"),
                std::fs::read_to_string(&canonical).expect("readable"),
            );
        }

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn extracts_component_css() {
        let css = component_css("deck-card");
        assert!(css.contains("deck-card {"));
        assert!(css.contains("border-radius"));
        assert!(!css.contains("deck-grid {"));
    }
}
