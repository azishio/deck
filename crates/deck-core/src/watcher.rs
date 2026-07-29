//! File watching for hot reload (design doc 11.8).

use std::time::Duration;

use camino::{Utf8Path, Utf8PathBuf};
use notify::{EventKind, RecursiveMode};
use notify_debouncer_full::{DebounceEventResult, new_debouncer};
use sha2::Digest;
use tokio::sync::mpsc;

use crate::config::{
    ASSETS_DIR, COMPONENTS_DIR, CONFIG_FILE, DESIGN_DIR, LOCAL_CONFIG_FILE, SLIDES_DIR,
};
use crate::error::{Error, Result};
use crate::project::Project;

/// A classified file system change.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Change {
    /// A slide document changed in place.
    Slide { path: String },
    /// A slide was added, removed or renamed: the manifest must be rebuilt.
    SlideSet,
    /// A project stylesheet changed.
    Style { path: String },
    /// The Tailwind entry changed; it is inlined into every slide document.
    TailwindEntry { path: String },
    /// A component module changed, together with the tags it defines.
    Component { path: String, tags: Vec<String> },
    /// An asset referenced by slides changed.
    Asset { path: String },
    /// `deck.toml` or `deck.local.toml` changed.
    Config { path: String },
}

impl Change {
    /// Project-relative path of the file behind this change, when there is one.
    fn path(&self) -> Option<&str> {
        match self {
            Self::Slide { path }
            | Self::Style { path }
            | Self::TailwindEntry { path }
            | Self::Component { path, .. }
            | Self::Asset { path }
            | Self::Config { path } => Some(path),
            Self::SlideSet => None,
        }
    }
}

/// Remembers file contents so a reload is only reported for a real edit.
///
/// This matters because the dev server reads the project constantly (serving a
/// slide, rebuilding the manifest, re-reading `deck.toml`). Those reads update
/// access times, which the platform reports as modifications — without this
/// gate the server would notify itself in a loop.
#[derive(Debug, Default)]
struct ContentGate {
    digests: std::collections::HashMap<String, [u8; 32]>,
}

impl ContentGate {
    /// Record every watched file so an unchanged file never looks new.
    fn prime(project: &Project) -> Self {
        let mut gate = Self::default();
        let root = project.root();

        for dir in project.watched_dirs() {
            for entry in walkdir::WalkDir::new(&dir).into_iter().flatten() {
                if !entry.file_type().is_file() {
                    continue;
                }
                if let Some(path) = Utf8Path::from_path(entry.path())
                    && let Some(relative) = path.strip_prefix(root).ok().map(Utf8Path::as_str)
                {
                    gate.changed(root, relative);
                }
            }
        }
        for config in config_paths(root) {
            if let Some(relative) = config.strip_prefix(root).ok().map(Utf8Path::as_str) {
                gate.changed(root, relative);
            }
        }
        gate
    }

    /// True when the file's content differs from the last time it was seen.
    fn changed(&mut self, root: &Utf8Path, relative: &str) -> bool {
        let Ok(bytes) = std::fs::read(root.join(relative)) else {
            // Unreadable now: a removal is a change, anything else is not.
            return self.digests.remove(relative).is_some();
        };
        let digest: [u8; 32] = sha2::Sha256::digest(&bytes).into();
        self.digests.insert(relative.to_owned(), digest) != Some(digest)
    }
}

/// Watch the project and stream batches of classified changes.
///
/// The returned receiver stays open until the guard is dropped.
pub struct Watcher {
    _debouncer: notify_debouncer_full::Debouncer<
        notify::RecommendedWatcher,
        notify_debouncer_full::RecommendedCache,
    >,
    pub changes: mpsc::UnboundedReceiver<Vec<Change>>,
}

impl Watcher {
    pub fn start(project: &Project) -> Result<Self> {
        let (tx, rx) = mpsc::unbounded_channel();
        let root = project.root().to_path_buf();
        let tailwind_entry = project.config().tailwind.entry.as_str().replace('\\', "/");
        let mut gate = ContentGate::prime(project);
        let slides_dir = project.slides_dir();
        let mut known_slides = slide_set(&slides_dir);

        let mut debouncer =
            new_debouncer(Duration::from_millis(150), None, move |result: DebounceEventResult| {
                match result {
                    Ok(events) => {
                        let mut changes = Vec::new();
                        for event in &events {
                            if !is_content_event(event.kind) {
                                continue;
                            }
                            for path in &event.paths {
                                let Some(path) = Utf8Path::from_path(path) else {
                                    continue;
                                };
                                let Some(mut change) = classify(&root, path, &tailwind_entry)
                                else {
                                    continue;
                                };
                                if change
                                    .path()
                                    .is_some_and(|relative| !gate.changed(&root, relative))
                                {
                                    continue;
                                }
                                // Editors that save atomically replace the file
                                // rather than write into it, so the event kind
                                // cannot tell an edit from an add or a rename.
                                // Comparing the slide set can.
                                if matches!(change, Change::Slide { .. }) {
                                    let current = slide_set(&slides_dir);
                                    if current != known_slides {
                                        known_slides = current;
                                        change = Change::SlideSet;
                                    }
                                }
                                if !changes.contains(&change) {
                                    changes.push(change);
                                }
                            }
                        }
                        if !changes.is_empty() {
                            let _ = tx.send(changes);
                        }
                    }
                    Err(errors) => {
                        for error in errors {
                            tracing::warn!("file watcher error: {error}");
                        }
                    }
                }
            })
            .map_err(|error| Error::config(format!("could not start the file watcher: {error}")))?;

        for dir in project.watched_dirs() {
            if dir.is_dir() {
                debouncer
                    .watch(dir.as_std_path(), RecursiveMode::Recursive)
                    .map_err(|error| Error::config(format!("could not watch {dir}: {error}")))?;
            }
        }
        // Non-recursive so dist/ and .deck/ churn does not wake the watcher.
        debouncer.watch(project.root().as_std_path(), RecursiveMode::NonRecursive).map_err(
            |error| Error::config(format!("could not watch {}: {}", project.root(), error)),
        )?;

        Ok(Self { _debouncer: debouncer, changes: rx })
    }
}

/// Relative paths of every slide document, used to tell an edit from an
/// add, a removal or a rename.
fn slide_set(slides_dir: &Utf8Path) -> std::collections::BTreeSet<String> {
    crate::discovery::slide_files(slides_dir).unwrap_or_default().into_iter().collect()
}

/// Events that can represent a real edit.
///
/// Access and metadata-only events are ignored: reading a file updates its
/// access time, and the dev server reads the project on every request.
fn is_content_event(kind: EventKind) -> bool {
    use notify::event::ModifyKind;

    match kind {
        EventKind::Create(_) | EventKind::Remove(_) => true,
        EventKind::Modify(ModifyKind::Metadata(_)) => false,
        EventKind::Modify(_) => true,
        EventKind::Access(_) | EventKind::Other | EventKind::Any => false,
    }
}

fn classify(root: &Utf8Path, path: &Utf8Path, tailwind_entry: &str) -> Option<Change> {
    let relative = path.strip_prefix(root).ok()?.as_str().replace('\\', "/");
    if relative.is_empty() {
        return None;
    }
    if is_editor_noise(&relative) {
        return None;
    }

    if relative == CONFIG_FILE || relative == LOCAL_CONFIG_FILE {
        return Some(Change::Config { path: relative });
    }
    if relative == tailwind_entry {
        return Some(Change::TailwindEntry { path: relative });
    }

    let (dir, rest) = relative.split_once('/')?;
    match dir {
        SLIDES_DIR if rest.ends_with(".html") => Some(Change::Slide { path: relative }),
        SLIDES_DIR | ASSETS_DIR => Some(Change::Asset { path: relative }),
        DESIGN_DIR => Some(Change::Style { path: relative }),
        COMPONENTS_DIR => {
            let tags = custom_element_tags(path);
            Some(Change::Component { path: relative, tags })
        }
        _ => None,
    }
}

fn is_editor_noise(relative: &str) -> bool {
    let name = relative.rsplit('/').next().unwrap_or(relative);
    name.starts_with('.')
        || name.ends_with('~')
        || name.ends_with(".swp")
        || name.ends_with(".tmp")
        || relative.contains("/node_modules/")
}

/// Scan a module for `customElements.define("tag-name"` so hot reload can
/// target only the iframes that actually use the component.
pub fn custom_element_tags(path: &Utf8Path) -> Vec<String> {
    let Ok(source) = std::fs::read_to_string(path) else {
        return Vec::new();
    };
    tags_in_source(&source)
}

fn tags_in_source(source: &str) -> Vec<String> {
    const NEEDLES: [&str; 2] = ["customElements.define(", "define("];
    let mut tags = Vec::new();

    for needle in NEEDLES {
        let mut cursor = 0;
        while let Some(found) = source[cursor..].find(needle) {
            let start = cursor + found + needle.len();
            cursor = start;
            let rest = source[start..].trim_start();
            let Some(quote) = rest.chars().next().filter(|c| matches!(c, '"' | '\'' | '`')) else {
                continue;
            };
            let body = &rest[quote.len_utf8()..];
            let Some(end) = body.find(quote) else { continue };
            let tag = &body[..end];
            if tag.contains('-')
                && tag.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
                && !tags.iter().any(|existing| existing == tag)
            {
                tags.push(tag.to_owned());
            }
        }
    }
    tags
}

/// Absolute paths of the config files, used to decide whether to reload.
pub fn config_paths(root: &Utf8Path) -> [Utf8PathBuf; 2] {
    [root.join(CONFIG_FILE), root.join(LOCAL_CONFIG_FILE)]
}

#[cfg(test)]
mod tests {
    use super::*;

    const TAILWIND: &str = "design/tailwind.css";

    #[test]
    fn classifies_project_paths() {
        let root = Utf8Path::new("/deck");
        assert_eq!(
            classify(root, Utf8Path::new("/deck/slides/00-a.html"), TAILWIND),
            Some(Change::Slide { path: "slides/00-a.html".into() })
        );
        assert_eq!(
            classify(root, Utf8Path::new("/deck/design/theme.css"), TAILWIND),
            Some(Change::Style { path: "design/theme.css".into() })
        );
        assert_eq!(
            classify(root, Utf8Path::new("/deck/deck.toml"), TAILWIND),
            Some(Change::Config { path: "deck.toml".into() })
        );
        assert_eq!(
            classify(root, Utf8Path::new("/deck/design/tailwind.css"), TAILWIND),
            Some(Change::TailwindEntry { path: "design/tailwind.css".into() })
        );
        assert_eq!(classify(root, Utf8Path::new("/deck/dist/index.html"), TAILWIND), None);
    }

    #[test]
    fn metadata_only_events_are_ignored() {
        use notify::event::{CreateKind, DataChange, MetadataKind, ModifyKind};

        assert!(is_content_event(EventKind::Modify(ModifyKind::Data(DataChange::Content))));
        assert!(is_content_event(EventKind::Create(CreateKind::File)));
        // Reading a file bumps its access time; that must not look like an edit.
        assert!(!is_content_event(EventKind::Modify(ModifyKind::Metadata(
            MetadataKind::AccessTime
        ))));
        assert!(!is_content_event(EventKind::Access(notify::event::AccessKind::Read)));
    }

    #[test]
    fn finds_custom_element_tags() {
        let source = r#"
            customElements.define("rccs-metric-card", MetricCard);
            define('rccs-node-diagram', NodeDiagram);
            define("nodash", Nope);
        "#;
        assert_eq!(tags_in_source(source), ["rccs-metric-card", "rccs-node-diagram"]);
    }
}
