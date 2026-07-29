//! Static build (design doc 19). The output runs on any static HTTP server and
//! needs neither Node.js nor the deck CLI.

use camino::{Utf8Path, Utf8PathBuf};
use serde::Serialize;
use sha2::{Digest, Sha256};

use crate::assets::{self, Page};
use crate::error::{Error, Result, write_file};
use crate::manifest::Manifest;
use crate::project::Project;
use crate::render::{self, RuntimeTags};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BuildSummary {
    pub output_dir: Utf8PathBuf,
    pub base_url: String,
    pub slides: usize,
    pub assets: usize,
}

pub fn run(project: &Project) -> Result<BuildSummary> {
    let config = project.config();
    let base = config.build.base_url.clone();
    let out = project.root().join(&config.build.output_dir);

    if out.exists() {
        std::fs::remove_dir_all(&out).map_err(|error| Error::io(&out, error))?;
    }
    std::fs::create_dir_all(&out).map_err(|error| Error::io(&out, error))?;

    // 1. assets, optionally fingerprinted
    let mut fingerprints = Vec::new();
    let asset_count = copy_tree(
        &project.assets_dir(),
        &out.join("assets"),
        "assets",
        &base,
        config.build.fingerprint_assets,
        &mut fingerprints,
    )?;
    copy_tree(
        &project.components_dir(),
        &out.join("components"),
        "components",
        &base,
        false,
        &mut Vec::new(),
    )?;
    copy_tree(&project.design_dir(), &out.join("design"), "design", &base, false, &mut Vec::new())?;

    // 2. embedded runtime assets
    for (route, bytes) in assets::EMBEDDED {
        write_file(&out.join("@deck").join(route), bytes)?;
    }

    // 3. generated assets
    write_file(&out.join("@deck/env.js"), assets::env_module(config))?;
    write_file(
        &out.join("@deck/design.css"),
        rewrite_css(&assets::design_css(project)?, &base, &fingerprints),
    )?;
    write_file(&out.join("@deck/components.js"), assets::components_js(project, &base))?;
    write_file(&out.join("@deck/tailwind.css"), assets::tailwind_input(project)?)?;

    let manifest = Manifest::build(&project.slides_dir(), 1)?;
    let manifest_json = manifest.to_json();
    write_file(&out.join("@deck/manifest.json"), &manifest_json)?;
    write_file(&out.join("deck-manifest.json"), &manifest_json)?;

    // 4. pages
    write_file(&out.join("index.html"), Page::Index.render(&base))?;
    write_file(&out.join("present/index.html"), Page::Present.render(&base))?;
    write_file(&out.join("presenter/index.html"), Page::Presenter.render(&base))?;
    write_file(&out.join("print/index.html"), Page::Print.render(&base))?;

    // 5. slides, one directory per stable id
    let tailwind = assets::tailwind_input(project)?;
    let tags = RuntimeTags { base_url: &base, tailwind_css: &tailwind };
    let slides_dir = project.slides_dir();
    for slide in &manifest.slides {
        let source = std::fs::read_to_string(slides_dir.join(&slide.path))
            .map_err(|error| Error::io(slides_dir.join(&slide.path), error))?;
        let html = render::rewrite_urls(
            &render::ensure_runtime_tags(&source, &tags),
            &base,
            &fingerprints,
        );
        write_file(&out.join("slides").join(&slide.id).join("index.html"), html)?;
    }

    Ok(BuildSummary {
        output_dir: out,
        base_url: base,
        slides: manifest.slides.len(),
        assets: asset_count,
    })
}

/// Copy a project directory into the build output.
///
/// When `fingerprint` is set, file names gain a content hash and the mapping
/// from the original URL is recorded so references can be rewritten.
fn copy_tree(
    source_dir: &Utf8Path,
    target_dir: &Utf8Path,
    url_prefix: &str,
    base_url: &str,
    fingerprint: bool,
    fingerprints: &mut Vec<(String, String)>,
) -> Result<usize> {
    if !source_dir.is_dir() {
        return Ok(0);
    }

    let mut copied = 0;
    for entry in walkdir::WalkDir::new(source_dir).into_iter().flatten() {
        if !entry.file_type().is_file() {
            continue;
        }
        let Some(path) = Utf8Path::from_path(entry.path()) else { continue };
        let Ok(relative) = path.strip_prefix(source_dir) else { continue };
        let bytes = std::fs::read(path).map_err(|error| Error::io(path, error))?;

        let relative = if fingerprint {
            let hashed = fingerprint_name(relative, &bytes);
            fingerprints.push((
                format!("/{url_prefix}/{relative}"),
                format!("{base_url}{url_prefix}/{hashed}"),
            ));
            hashed
        } else {
            relative.to_path_buf()
        };

        write_file(&target_dir.join(&relative), &bytes)?;
        copied += 1;
    }
    Ok(copied)
}

fn fingerprint_name(relative: &Utf8Path, bytes: &[u8]) -> Utf8PathBuf {
    let digest = hex::encode(Sha256::digest(bytes));
    let short = &digest[..8];
    let stem = relative.file_stem().unwrap_or("asset");
    let name = match relative.extension() {
        Some(extension) => format!("{stem}.{short}.{extension}"),
        None => format!("{stem}.{short}"),
    };
    relative.parent().map_or_else(|| Utf8PathBuf::from(&name), |parent| parent.join(&name))
}

fn rewrite_css(css: &str, base_url: &str, fingerprints: &[(String, String)]) -> String {
    let mut out = css.to_owned();
    for (from, to) in fingerprints {
        out = out.replace(from, to);
    }
    if base_url == "/" {
        return out;
    }
    out.replace("url(/", &format!("url({base_url}"))
        .replace("url(\"/", &format!("url(\"{base_url}"))
        .replace("url('/", &format!("url('{base_url}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fingerprints_keep_the_extension() {
        let name = fingerprint_name(Utf8Path::new("images/logo.png"), b"hello");
        assert!(name.as_str().starts_with("images/logo."));
        assert!(name.as_str().ends_with(".png"));
    }

    #[test]
    fn css_urls_are_rewritten() {
        let map = vec![("/assets/f.woff2".to_owned(), "/assets/f.1234abcd.woff2".to_owned())];
        let out = rewrite_css("@font-face { src: url(/assets/f.woff2); }", "/", &map);
        assert!(out.contains("/assets/f.1234abcd.woff2"));
    }
}
