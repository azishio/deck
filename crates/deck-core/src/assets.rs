//! Web assets embedded into the binary, plus the ones generated per project.
//!
//! Everything under `/@deck/` is served from here, so normal use never needs
//! Node.js or a network fetch (design doc 18).

use camino::Utf8Path;

use crate::config::{AnimationEngine, Config};
use crate::error::{Result, read_to_string};
use crate::project::Project;

macro_rules! embedded {
    ($($route:literal => $file:literal),* $(,)?) => {
        /// `(/@deck/… route, bytes)` for every embedded asset.
        pub const EMBEDDED: &[(&str, &[u8])] = &[
            $(($route, include_bytes!(concat!("../../../web/", $file)))),*
        ];
    };
}

embedded! {
    "boot.js" => "slide-runtime/boot.js",
    "runtime.js" => "slide-runtime/runtime.js",
    "shell.js" => "presenter/shell.js",
    "shell.css" => "presenter/shell.css",
    "present.js" => "presenter/present.js",
    "presenter.js" => "presenter/presenter.js",
    "index.js" => "presenter/index.js",
    "print.js" => "print/print.js",
    "print.css" => "print/print.css",
    "vendor/animejs.js" => "vendor/animejs.js",
    "vendor/animejs.LICENSE.md" => "vendor/animejs.LICENSE.md",
    "vendor/tailwind.js" => "vendor/tailwind.js",
    "vendor/tailwind.LICENSE.md" => "vendor/tailwind.LICENSE.md",
}

/// Design system pieces merged into `/@deck/design.css`.
const CSS_RESET: &str = include_str!("../../../web/design-system/reset.css");
const CSS_TOKENS: &str = include_str!("../../../web/themes/tokens.css");
const CSS_BASE: &str = include_str!("../../../web/design-system/base.css");
const CSS_COMPONENTS: &str = include_str!("../../../web/design-system/components.css");

/// Cascade order for every layer used by a slide, lowest priority first.
///
/// `base` (Tailwind's preflight), `theme`, `components` and `utilities` are
/// Tailwind's top-level layers; `deck.*` are sub-layers of one `deck` layer, so
/// the two lists must be declared separately — listing `deck.reset` alongside
/// `base` would place the whole `deck` layer wherever its first sub-layer
/// appears, and preflight would then outrank the entire design system.
///
/// Declaring the order in both `design.css` and the Tailwind entry keeps the
/// cascade independent of which stylesheet the browser applies first.
const LAYER_ORDER: &str = concat!(
    "@layer base, theme, deck, project, slide, components, utilities;\n",
    "@layer deck.reset, deck.tokens, deck.base, deck.components;\n",
);

/// How long a slide waits for Tailwind's first in-browser compilation.
const TAILWIND_COMPILE_TIMEOUT_MS: u32 = 5_000;

/// Maps deck tokens onto the Tailwind theme; prepended to the Tailwind entry.
const TAILWIND_THEME: &str = include_str!("../../../web/design-system/tailwind-theme.css");

/// Built-in Custom Elements, served as part of `/@deck/components.js`.
const BUILT_IN_COMPONENTS: &str = include_str!("../../../web/design-system/components.js");

/// Layout probe injected by `deck check`.
pub const CHECK_PROBE: &str = include_str!("../../../web/check/probe.js");

/// Page shells. `__DECK_BASE__` is replaced with the deck base URL.
const PAGE_INDEX: &str = include_str!("../../../web/presenter/index.html");
const PAGE_PRESENT: &str = include_str!("../../../web/presenter/present.html");
const PAGE_PRESENTER: &str = include_str!("../../../web/presenter/presenter.html");
const PAGE_PRINT: &str = include_str!("../../../web/print/print.html");

/// Names of the built-in components, used by `deck component list`.
pub const BUILT_IN_COMPONENT_NAMES: &[&str] = &[
    "deck-slide",
    "deck-heading",
    "deck-grid",
    "deck-stack",
    "deck-card",
    "deck-callout",
    "deck-stat",
    "deck-figure",
    "deck-code",
    "deck-notes",
];

/// Look up an embedded asset by its path below `/@deck/`.
pub fn embedded(path: &str) -> Option<&'static [u8]> {
    EMBEDDED.iter().find(|(route, _)| *route == path).map(|(_, bytes)| *bytes)
}

pub fn mime_for(path: &str) -> &'static str {
    match Utf8Path::new(path).extension() {
        Some("js" | "mjs") => "text/javascript; charset=utf-8",
        Some("css") => "text/css; charset=utf-8",
        Some("html") => "text/html; charset=utf-8",
        Some("json") => "application/json; charset=utf-8",
        Some("svg") => "image/svg+xml",
        Some("md") => "text/markdown; charset=utf-8",
        _ => "application/octet-stream",
    }
}

/* -------------------------------------------------------------------------- */
/* pages                                                                       */
/* -------------------------------------------------------------------------- */

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Page {
    Index,
    Present,
    Presenter,
    Print,
}

impl Page {
    pub fn render(self, base_url: &str) -> String {
        let template = match self {
            Self::Index => PAGE_INDEX,
            Self::Present => PAGE_PRESENT,
            Self::Presenter => PAGE_PRESENTER,
            Self::Print => PAGE_PRINT,
        };
        template.replace("__DECK_BASE__", base_url)
    }
}

/* -------------------------------------------------------------------------- */
/* generated assets                                                            */
/* -------------------------------------------------------------------------- */

/// `/@deck/env.js` — the slice of configuration the browser needs.
pub fn env_module(config: &Config) -> String {
    let value = serde_json::json!({
        "deck": {
            "title": config.deck.title,
            "author": config.deck.author,
            "lang": config.deck.lang,
        },
        "canvas": {
            "width": config.canvas.width,
            "height": config.canvas.height,
            "safeArea": {
                "top": config.canvas.safe_top(),
                "right": config.canvas.safe_right(),
                "bottom": config.canvas.safe_bottom(),
                "left": config.canvas.safe_left(),
            },
        },
        "server": {
            "preload": config.server.preload,
            "hotReload": config.server.hot_reload,
        },
        "animation": {
            "engine": match config.animation.engine {
                AnimationEngine::Animejs => "animejs",
                AnimationEngine::None => "none",
            },
            "reducedMotion": format!("{:?}", config.animation.reduced_motion).to_lowercase(),
        },
        "tailwind": {
            "entry": config.tailwind.entry,
            "preflight": config.tailwind.preflight,
        },
        "tailwindTimeoutMs": TAILWIND_COMPILE_TIMEOUT_MS,
        "print": {
            "steps": config.print.steps.as_str(),
            "preflight": config.print.preflight,
            "showNotes": config.print.show_notes,
        },
        "check": {
            "overflowTolerancePx": config.check.overflow_tolerance_px,
            "minFontPx": config.check.min_font_px,
        },
        "readyTimeoutMs": config.check.timeout_ms,
    });

    format!(
        "// Generated by deck. Do not edit.\nexport const env = Object.freeze({});\nexport default env;\n",
        serde_json::to_string_pretty(&value).unwrap_or_else(|_| "{}".into())
    )
}

/// Tailwind entry compiled by the vendored browser build inside every slide.
///
/// Order matters: preflight (the reset) is imported first, then the theme, then
/// the utilities.
pub fn tailwind_input(project: &Project) -> Result<String> {
    let config = project.config();
    let mut css = String::from("/* Generated by deck. */\n");
    css.push_str(LAYER_ORDER);

    // Reset first: preflight normalises the document before any token, base or
    // component style is applied. `@layer base` keeps it below the deck design
    // system in the cascade even though it is imported first.
    if config.tailwind.preflight {
        css.push_str("@import \"tailwindcss/preflight.css\" layer(base);\n");
    }
    css.push_str("@import \"tailwindcss/theme.css\" layer(theme);\n");
    css.push_str("@import \"tailwindcss/utilities.css\" layer(utilities);\n\n");
    css.push_str(TAILWIND_THEME);

    let entry = project.root().join(&config.tailwind.entry);
    if entry.is_file() {
        css.push('\n');
        css.push_str(&read_to_string(&entry)?);
    }
    Ok(css)
}

/// `/@deck/design.css` — built-in layers followed by the project stylesheets.
pub fn design_css(project: &Project) -> Result<String> {
    let config = project.config();
    let mut css = String::with_capacity(32 * 1024);

    css.push_str("/* Generated by deck. Do not edit. */\n");
    css.push_str(LAYER_ORDER);
    css.push('\n');
    css.push_str(CSS_RESET);
    css.push('\n');
    css.push_str(CSS_TOKENS);
    css.push('\n');

    // Canvas values come from deck.toml, so they are emitted rather than embedded.
    css.push_str(&format!(
        "@layer deck.tokens {{\n  :root {{\n    --deck-canvas-width: {}px;\n    --deck-canvas-height: {}px;\n    --deck-safe-top: {}px;\n    --deck-safe-right: {}px;\n    --deck-safe-bottom: {}px;\n    --deck-safe-left: {}px;\n  }}\n}}\n\n",
        config.canvas.width,
        config.canvas.height,
        config.canvas.safe_top(),
        config.canvas.safe_right(),
        config.canvas.safe_bottom(),
        config.canvas.safe_left(),
    ));

    css.push_str(CSS_BASE);
    css.push('\n');
    css.push_str(CSS_COMPONENTS);
    css.push('\n');

    for style in &config.theme.styles {
        let path = project.root().join(style);
        if !path.is_file() {
            continue;
        }
        let contents = read_to_string(&path)?;
        css.push_str(&format!("\n/* {style} */\n@layer project {{\n"));
        css.push_str(&contents);
        css.push_str("\n}\n");
    }

    Ok(css)
}

/// `/@deck/components.js` — built-in elements plus the project entry point.
pub fn components_js(project: &Project, base_url: &str) -> String {
    let mut js = String::from("// Generated by deck. Do not edit.\n");
    js.push_str(BUILT_IN_COMPONENTS);

    let entry = project.components_entry();
    if entry.is_file() {
        let relative = project
            .relative(&entry)
            .unwrap_or_else(|| project.config().components.entry.as_str().to_owned());
        js.push_str(&format!("\n// project components\nimport \"{base_url}{relative}\";\n"));
    }
    js
}

/// Stylesheets that `deck check` should treat as project-owned CSS.
pub fn project_style_paths(project: &Project) -> Vec<camino::Utf8PathBuf> {
    project
        .config()
        .theme
        .styles
        .iter()
        .map(|style| project.root().join(style))
        .filter(|path| path.is_file())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_embedded_asset_is_non_empty() {
        for (route, bytes) in EMBEDDED {
            assert!(!bytes.is_empty(), "{route} が空です");
        }
    }

    #[test]
    fn pages_have_their_base_url_substituted() {
        let html = Page::Present.render("/deck/");
        assert!(html.contains("/deck/@deck/present.js"));
        assert!(!html.contains("__DECK_BASE__"));
    }

    #[test]
    fn env_module_is_a_module() {
        let module = env_module(&Config::default());
        assert!(module.contains("export const env"));
        assert!(module.contains("\"width\": 1280"));
    }
}
