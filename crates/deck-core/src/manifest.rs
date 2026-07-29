//! The internal slide manifest (design doc 13).
//!
//! Slide order comes from the file system; stable ids, titles, notes and step
//! counts come from the HTML itself.

use std::collections::HashSet;
use std::sync::LazyLock;

use camino::{Utf8Path, Utf8PathBuf};
use scraper::{Html, Selector};
use serde::Serialize;

use crate::discovery;
use crate::error::{Result, read_to_string};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Slide {
    /// Stable id used by URLs, presenter sync and hot reload.
    pub id: String,
    /// Path relative to `slides/`.
    pub path: String,
    pub title: String,
    pub order: usize,
    /// Highest `data-step` value found in the document.
    pub step_count: u32,
    /// Rendered contents of `<deck-notes>`, used by the presenter view.
    pub notes: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct Manifest {
    pub revision: u64,
    pub slides: Vec<Slide>,
}

impl Manifest {
    /// Discover `slides/**/*.html` and parse each document.
    pub fn build(slides_dir: &Utf8Path, revision: u64) -> Result<Self> {
        let mut slides = Vec::new();
        let mut seen_ids = HashSet::new();

        for (order, relative) in discovery::slide_files(slides_dir)?.into_iter().enumerate() {
            let absolute = slides_dir.join(&relative);
            let document = SlideDocument::parse(&absolute, &relative)?;

            let mut id = document.id.clone();
            if !seen_ids.insert(id.clone()) {
                // Duplicate ids are reported by `deck check`; the manifest must
                // still expose addressable slides.
                let fallback = discovery::id_from_path(&relative);
                tracing::warn!("duplicate slide id {id} ({relative}); using {fallback} instead");
                id = fallback;
                let mut suffix = 2;
                while !seen_ids.insert(id.clone()) {
                    id = format!("{}-{suffix}", discovery::id_from_path(&relative));
                    suffix += 1;
                }
            }

            slides.push(Slide {
                id,
                path: relative,
                title: document.title,
                order,
                step_count: document.step_count,
                notes: document.notes,
            });
        }

        Ok(Self { revision, slides })
    }

    pub fn slide(&self, id: &str) -> Option<&Slide> {
        self.slides.iter().find(|slide| slide.id == id)
    }

    pub fn slide_by_path(&self, relative_path: &str) -> Option<&Slide> {
        self.slides.iter().find(|slide| slide.path == relative_path)
    }

    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).unwrap_or_else(|_| "{}".into())
    }
}

/* -------------------------------------------------------------------------- */
/* HTML parsing                                                                */
/* -------------------------------------------------------------------------- */

static TITLE: LazyLock<Selector> = LazyLock::new(|| Selector::parse("title").unwrap());
static DECK_SLIDE: LazyLock<Selector> = LazyLock::new(|| Selector::parse("deck-slide").unwrap());
static DECK_NOTES: LazyLock<Selector> = LazyLock::new(|| Selector::parse("deck-notes").unwrap());
static STEPPED: LazyLock<Selector> = LazyLock::new(|| Selector::parse("[data-step]").unwrap());
static HEADING: LazyLock<Selector> =
    LazyLock::new(|| Selector::parse("deck-heading, h1, h2").unwrap());

/// Everything the manifest and the static checks need from one slide document.
#[derive(Debug, Clone)]
pub struct SlideDocument {
    pub path: Utf8PathBuf,
    pub relative: String,
    pub id: String,
    pub title: String,
    pub step_count: u32,
    pub notes: String,
    pub has_deck_slide: bool,
    pub has_title_element: bool,
    pub source: String,
}

impl SlideDocument {
    pub fn parse(path: &Utf8Path, relative: &str) -> Result<Self> {
        let source = read_to_string(path)?;
        Ok(Self::from_source(path.to_path_buf(), relative.to_owned(), source))
    }

    pub fn from_source(path: Utf8PathBuf, relative: String, source: String) -> Self {
        let html = Html::parse_document(&source);

        let slide_element = html.select(&DECK_SLIDE).next();
        let id = slide_element
            .and_then(|element| element.value().attr("id"))
            .map(str::to_owned)
            .filter(|id| !id.is_empty())
            .unwrap_or_else(|| discovery::id_from_path(&relative));

        let title_element = html.select(&TITLE).next();
        let title = title_element
            .map(|element| element.text().collect::<String>().trim().to_owned())
            .filter(|title| !title.is_empty())
            .or_else(|| {
                html.select(&HEADING)
                    .next()
                    .map(|element| collapse_whitespace(&element.text().collect::<String>()))
                    .filter(|text| !text.is_empty())
            })
            .unwrap_or_else(|| id.clone());

        let step_count = html
            .select(&STEPPED)
            .filter_map(|element| element.value().attr("data-step"))
            .filter_map(|value| value.trim().parse::<u32>().ok())
            .max()
            .unwrap_or(0);

        let notes = html
            .select(&DECK_NOTES)
            .map(|element| element.inner_html().trim().to_owned())
            .collect::<Vec<_>>()
            .join("\n");

        Self {
            path,
            relative,
            id,
            title,
            step_count,
            notes,
            has_deck_slide: slide_element.is_some(),
            has_title_element: title_element.is_some(),
            source,
        }
    }
}

fn collapse_whitespace(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(source: &str) -> SlideDocument {
        SlideDocument::from_source(
            Utf8PathBuf::from("slides/00-a.html"),
            "00-a.html".to_owned(),
            source.to_owned(),
        )
    }

    #[test]
    fn reads_id_title_steps_and_notes() {
        let document = parse(
            r#"<!doctype html><html><head><title>Architecture</title></head>
               <body><deck-slide id="architecture">
                 <p data-step="1">a</p><p data-step="3">b</p>
                 <deck-notes>speaker <b>notes</b></deck-notes>
               </deck-slide></body></html>"#,
        );
        assert_eq!(document.id, "architecture");
        assert_eq!(document.title, "Architecture");
        assert_eq!(document.step_count, 3);
        assert_eq!(document.notes, "speaker <b>notes</b>");
        assert!(document.has_deck_slide);
    }

    #[test]
    fn id_falls_back_to_the_relative_path() {
        let document = SlideDocument::from_source(
            Utf8PathBuf::from("slides/20-architecture/10-ingestion.html"),
            "20-architecture/10-ingestion.html".to_owned(),
            "<deck-slide></deck-slide>".to_owned(),
        );
        assert_eq!(document.id, "20-architecture/10-ingestion");
    }

    #[test]
    fn title_falls_back_to_the_first_heading() {
        let document =
            parse("<deck-slide id=\"x\"><deck-heading>A heading</deck-heading></deck-slide>");
        assert_eq!(document.title, "A heading");
        assert!(!document.has_title_element);
    }
}
