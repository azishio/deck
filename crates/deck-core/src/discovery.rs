//! Directory-based slide discovery (design doc 4.2).
//!
//! `slides/**/*.html` is walked recursively and ordered by the plain
//! lexicographic order of the normalised relative path. Natural sort is
//! deliberately not used.

use camino::Utf8Path;
use walkdir::WalkDir;

use crate::error::Result;

/// Relative `/`-separated paths of every slide document, in presentation order.
pub fn slide_files(slides_dir: &Utf8Path) -> Result<Vec<String>> {
    if !slides_dir.is_dir() {
        return Ok(Vec::new());
    }

    let mut files = Vec::new();
    for entry in WalkDir::new(slides_dir).follow_links(false).sort_by_file_name() {
        let entry = match entry {
            Ok(entry) => entry,
            Err(error) => {
                tracing::warn!("slides の探索中にエラー: {error}");
                continue;
            }
        };
        if !entry.file_type().is_file() {
            continue;
        }
        let Ok(path) = Utf8Path::from_path(entry.path()).ok_or(()) else {
            tracing::warn!("UTF-8でないパスを無視します: {}", entry.path().display());
            continue;
        };
        if path.extension() != Some("html") {
            continue;
        }
        if path.file_name().is_some_and(|name| name.starts_with('_') || name.starts_with('.')) {
            continue;
        }
        let Ok(relative) = path.strip_prefix(slides_dir) else {
            continue;
        };
        files.push(relative.as_str().replace('\\', "/"));
    }

    files.sort();
    Ok(files)
}

/// Stable slide id derived from a relative path (design doc 4.3).
pub fn id_from_path(relative_path: &str) -> String {
    relative_path.strip_suffix(".html").unwrap_or(relative_path).to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ordering_is_lexicographic() {
        let mut files = vec![
            "20-architecture/10-ingestion.html".to_owned(),
            "00-title.html".to_owned(),
            "20-architecture/00-overview.html".to_owned(),
            "90-summary.html".to_owned(),
            "10-background.html".to_owned(),
        ];
        files.sort();
        assert_eq!(
            files,
            [
                "00-title.html",
                "10-background.html",
                "20-architecture/00-overview.html",
                "20-architecture/10-ingestion.html",
                "90-summary.html",
            ]
        );
    }

    #[test]
    fn id_drops_the_extension() {
        assert_eq!(
            id_from_path("20-architecture/10-ingestion.html"),
            "20-architecture/10-ingestion"
        );
    }
}
