//! Static, runtime and layout checks (design doc 16).

use std::collections::{BTreeMap, HashSet};
use std::sync::LazyLock;
use std::time::Duration;

use camino::{Utf8Path, Utf8PathBuf};
use scraper::{Html, Selector};
use serde::Serialize;

use crate::assets;
use crate::browser::BrowserSession;
use crate::config::{NetworkPolicy, Severity};
use crate::error::{Error, Result};
use crate::manifest::{Manifest, SlideDocument};
use crate::project::Project;
use crate::server::Server;

#[derive(Debug, Clone, Copy, Serialize, serde::Deserialize, PartialEq)]
pub struct Rect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Diagnostic {
    pub rule: String,
    pub severity: Severity,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub slide_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub selector: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rect: Option<Rect>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub screenshot: Option<String>,
}

impl Diagnostic {
    fn new(rule: impl Into<String>, severity: Severity, message: impl Into<String>) -> Self {
        Self {
            rule: rule.into(),
            severity,
            message: message.into(),
            slide_id: None,
            source_path: None,
            selector: None,
            rect: None,
            screenshot: None,
        }
    }

    fn at(mut self, slide_id: &str, source_path: &str) -> Self {
        self.slide_id = Some(slide_id.to_owned());
        self.source_path = Some(source_path.to_owned());
        self
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Report {
    pub deck_title: String,
    pub slides_checked: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub chromium_version: Option<String>,
    pub diagnostics: Vec<Diagnostic>,
}

impl Report {
    pub fn errors(&self) -> usize {
        self.count(Severity::Error)
    }

    pub fn warnings(&self) -> usize {
        self.count(Severity::Warning)
    }

    fn count(&self, severity: Severity) -> usize {
        self.diagnostics.iter().filter(|item| item.severity == severity).count()
    }

    /// `Err(CheckViolations)` when at least one error was reported.
    pub fn into_result(self) -> Result<Self> {
        let (errors, warnings) = (self.errors(), self.warnings());
        if errors > 0 { Err(Error::CheckViolations { errors, warnings }) } else { Ok(self) }
    }
}

#[derive(Debug, Clone, Default)]
pub struct CheckOptions {
    /// Slide ids to check; empty means every slide.
    pub slides: Vec<String>,
    /// Only check slides whose content changed since the last run.
    pub changed_only: bool,
    /// Skip Chromium entirely.
    pub static_only: bool,
    /// Save a screenshot per slide under `.deck/screenshots/`.
    pub screenshots: bool,
}

/* -------------------------------------------------------------------------- */
/* entry point                                                                 */
/* -------------------------------------------------------------------------- */

pub async fn run(project: &Project, options: &CheckOptions) -> Result<Report> {
    let manifest = Manifest::build(&project.slides_dir(), 1)?;
    let targets = select_slides(project, &manifest, options)?;

    let mut diagnostics = static_checks(project, &manifest, &targets)?;
    let mut chromium_version = None;

    if !options.static_only && !targets.is_empty() {
        let (browser_diagnostics, version) =
            runtime_checks(project, &manifest, &targets, options).await?;
        diagnostics.extend(browser_diagnostics);
        chromium_version = Some(version);
    }

    diagnostics.retain(|diagnostic| diagnostic.severity != Severity::Off);
    diagnostics.sort_by(|a, b| {
        (a.slide_id.as_deref(), &a.rule, &a.message).cmp(&(
            b.slide_id.as_deref(),
            &b.rule,
            &b.message,
        ))
    });

    if options.changed_only {
        record_cache(project, &targets)?;
    }

    Ok(Report {
        deck_title: project.config().deck.title.clone(),
        slides_checked: targets.len(),
        chromium_version,
        diagnostics,
    })
}

fn select_slides(
    project: &Project,
    manifest: &Manifest,
    options: &CheckOptions,
) -> Result<Vec<SlideDocument>> {
    let ignored: HashSet<&str> =
        project.config().check.ignore.slides.iter().map(String::as_str).collect();
    let cache = if options.changed_only { load_cache(project) } else { BTreeMap::new() };
    let slides_dir = project.slides_dir();

    let mut selected = Vec::new();
    for slide in &manifest.slides {
        if ignored.contains(slide.id.as_str()) {
            continue;
        }
        if !options.slides.is_empty()
            && !options.slides.iter().any(|wanted| *wanted == slide.id || *wanted == slide.path)
        {
            continue;
        }
        let document = SlideDocument::parse(&slides_dir.join(&slide.path), &slide.path)?;
        if options.changed_only
            && cache.get(&slide.path).is_some_and(|hash| *hash == digest(&document.source))
        {
            continue;
        }
        selected.push(document);
    }
    Ok(selected)
}

/* -------------------------------------------------------------------------- */
/* static checks (design doc 16.2)                                             */
/* -------------------------------------------------------------------------- */

static ALL_ELEMENTS: LazyLock<Selector> = LazyLock::new(|| Selector::parse("*").unwrap());

fn static_checks(
    project: &Project,
    manifest: &Manifest,
    targets: &[SlideDocument],
) -> Result<Vec<Diagnostic>> {
    let config = project.config();
    let rules = &config.check.rules;
    let mut diagnostics = Vec::new();

    // duplicate slide ids across the deck
    let mut seen: BTreeMap<&str, &str> = BTreeMap::new();
    for document in targets {
        if let Some(previous) = seen.insert(document.id.as_str(), document.relative.as_str()) {
            diagnostics.push(
                Diagnostic::new(
                    "duplicate_slide_id",
                    rules.severity("duplicate_slide_id"),
                    format!("slide id '{}' が {previous} と重複しています", document.id),
                )
                .at(&document.id, &document.relative),
            );
        }
    }

    let known_components = known_component_names(project);

    for document in targets {
        let slide_id = document.id.as_str();
        let source_path = document.relative.as_str();
        let html = Html::parse_document(&document.source);

        if !document.has_title_element {
            diagnostics.push(
                Diagnostic::new(
                    "missing_title",
                    rules.severity("missing_title"),
                    "<title> がありません",
                )
                .at(slide_id, source_path),
            );
        }
        if !document.has_deck_slide {
            diagnostics.push(
                Diagnostic::new(
                    "missing_deck_slide",
                    rules.severity("missing_deck_slide"),
                    "<deck-slide> がありません",
                )
                .at(slide_id, source_path),
            );
        }

        let mut html_ids = HashSet::new();
        let mut reported_tags = HashSet::new();
        for element in html.select(&ALL_ELEMENTS) {
            let value = element.value();

            if let Some(id) = value.attr("id")
                && !html_ids.insert(id.to_owned())
            {
                diagnostics.push(
                    Diagnostic::new(
                        "duplicate_html_id",
                        rules.severity("duplicate_html_id"),
                        format!("id='{id}' が重複しています"),
                    )
                    .at(slide_id, source_path),
                );
            }

            let tag = value.name();
            if tag.contains('-') && reported_tags.insert(tag.to_owned()) {
                if !is_valid_custom_element_name(tag) {
                    diagnostics.push(
                        Diagnostic::new(
                            "invalid_component_name",
                            rules.severity("invalid_component_name"),
                            format!("Custom Element 名として不正です: <{tag}>"),
                        )
                        .at(slide_id, source_path),
                    );
                } else if !known_components.contains(tag) {
                    diagnostics.push(
                        Diagnostic::new(
                            "invalid_component_name",
                            rules.severity("invalid_component_name"),
                            format!("定義が見つからないコンポーネントです: <{tag}>"),
                        )
                        .at(slide_id, source_path),
                    );
                }
            }
        }

        for url in collect_urls(&html, &document.source) {
            check_url(project, manifest, document, &url, &mut diagnostics);
        }
    }

    Ok(diagnostics)
}

fn is_valid_custom_element_name(name: &str) -> bool {
    let mut chars = name.chars();
    let Some(first) = chars.next() else { return false };
    if !first.is_ascii_lowercase() {
        return false;
    }
    if !name.contains('-') {
        return false;
    }
    name.chars()
        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '_' || c == '.')
}

/// Built-in components plus every tag defined under `components/`.
fn known_component_names(project: &Project) -> HashSet<String> {
    let mut names: HashSet<String> =
        assets::BUILT_IN_COMPONENT_NAMES.iter().map(|name| (*name).to_owned()).collect();

    let components_dir = project.components_dir();
    if components_dir.is_dir() {
        for entry in walkdir::WalkDir::new(&components_dir).into_iter().flatten() {
            let Some(path) = Utf8Path::from_path(entry.path()) else { continue };
            if path.extension().is_some_and(|extension| matches!(extension, "js" | "mjs")) {
                names.extend(crate::watcher::custom_element_tags(path));
            }
        }
    }
    names
}

/// URLs referenced from attributes and `url(...)` in inline CSS.
fn collect_urls(html: &Html, source: &str) -> Vec<String> {
    const URL_ATTRS: [&str; 5] = ["src", "href", "poster", "data-src", "srcset"];
    let mut urls = Vec::new();

    for element in html.select(&ALL_ELEMENTS) {
        for attribute in URL_ATTRS {
            let Some(value) = element.value().attr(attribute) else { continue };
            if attribute == "srcset" {
                urls.extend(
                    value
                        .split(',')
                        .filter_map(|candidate| candidate.split_whitespace().next())
                        .map(str::to_owned),
                );
            } else {
                urls.push(value.to_owned());
            }
        }
    }

    let mut rest = source;
    while let Some(start) = rest.find("url(") {
        rest = &rest[start + 4..];
        let Some(end) = rest.find(')') else { break };
        let raw = rest[..end].trim().trim_matches(['"', '\'']);
        if !raw.is_empty() {
            urls.push(raw.to_owned());
        }
        rest = &rest[end + 1..];
    }

    urls
}

fn check_url(
    project: &Project,
    manifest: &Manifest,
    document: &SlideDocument,
    url: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let rules = &project.config().check.rules;
    let slide_id = document.id.as_str();
    let source_path = document.relative.as_str();
    let url = url.trim();

    if url.is_empty()
        || url.starts_with('#')
        || url.starts_with("data:")
        || url.starts_with("mailto:")
        || url.starts_with("javascript:")
        || url.starts_with("blob:")
    {
        return;
    }

    if url.starts_with("http://") || url.starts_with("https://") || url.starts_with("//") {
        if project.config().check.external_network == NetworkPolicy::Deny {
            diagnostics.push(
                Diagnostic::new(
                    "external_url",
                    rules.severity("external_url"),
                    format!("外部URLを参照しています: {url}"),
                )
                .at(slide_id, source_path),
            );
        }
        return;
    }

    let path_only = url.split(['?', '#']).next().unwrap_or(url);

    if let Some(rest) = path_only.strip_prefix("/@deck/") {
        let known = assets::embedded(rest).is_some()
            || matches!(rest, "manifest.json" | "design.css" | "components.js" | "env.js");
        if !known {
            diagnostics.push(
                Diagnostic::new(
                    "invalid_local_url",
                    rules.severity("invalid_local_url"),
                    format!("予約URLが存在しません: {url}"),
                )
                .at(slide_id, source_path),
            );
        }
        return;
    }

    let resolved: Option<Utf8PathBuf> = if let Some(rest) = path_only.strip_prefix("/assets/") {
        Some(project.assets_dir().join(rest))
    } else if let Some(rest) = path_only.strip_prefix("/components/") {
        Some(project.components_dir().join(rest))
    } else if let Some(rest) = path_only.strip_prefix("/design/") {
        Some(project.design_dir().join(rest))
    } else if let Some(rest) = path_only.strip_prefix("/slides/") {
        if manifest.slide(rest).is_some() {
            return;
        }
        Some(project.slides_dir().join(rest))
    } else if path_only.starts_with('/') {
        diagnostics.push(
            Diagnostic::new(
                "invalid_local_url",
                rules.severity("invalid_local_url"),
                format!("配信されないパスを参照しています: {url}"),
            )
            .at(slide_id, source_path),
        );
        return;
    } else {
        document.path.parent().map(|parent| parent.join(path_only))
    };

    let Some(resolved) = resolved else { return };
    if !resolved.exists() {
        diagnostics.push(
            Diagnostic::new(
                "missing_file",
                rules.severity("missing_file"),
                format!("ファイルが存在しません: {url}"),
            )
            .at(slide_id, source_path),
        );
    }
}

/* -------------------------------------------------------------------------- */
/* runtime and layout checks (design doc 16.3, 16.4)                           */
/* -------------------------------------------------------------------------- */

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct ProbeResult {
    #[serde(default)]
    step_count: Option<u32>,
    #[serde(default)]
    diagnostics: Vec<ProbeDiagnostic>,
    #[serde(default)]
    runtime_diagnostics: Vec<ProbeDiagnostic>,
}

#[derive(Debug, serde::Deserialize)]
struct ProbeDiagnostic {
    rule: String,
    #[serde(default)]
    severity: Option<String>,
    message: String,
    #[serde(default)]
    selector: Option<String>,
    #[serde(default)]
    rect: Option<Rect>,
}

async fn runtime_checks(
    project: &Project,
    manifest: &Manifest,
    targets: &[SlideDocument],
    options: &CheckOptions,
) -> Result<(Vec<Diagnostic>, String)> {
    let config = project.config();
    let rules = &config.check.rules;
    let timeout = Duration::from_millis(config.check.timeout_ms);

    let server = Server::bind(project.clone()).await?;
    let origin = server.origin();
    let _server = server.spawn();

    let browser = BrowserSession::launch(&config.browser, config.canvas).await?;
    let version = browser.version().await.unwrap_or_else(|_| "unknown".into());

    let probe = assets::CHECK_PROBE.replace("__DECK_CHECK_CONFIG__", &probe_config(project));
    let screenshots_dir = project.work_dir().join("screenshots");
    let mut diagnostics = Vec::new();

    for document in targets {
        let slide_id = document.id.as_str();
        let source_path = document.relative.as_str();
        let url = format!("{origin}/slides/{slide_id}?deck-mode=check&step=final");

        let session = match browser.open(&url).await {
            Ok(session) => session,
            Err(error) => {
                diagnostics.push(
                    Diagnostic::new(
                        "ready_timeout",
                        rules.severity("ready_timeout"),
                        error.to_string(),
                    )
                    .at(slide_id, source_path),
                );
                continue;
            }
        };

        let ready = session
            .wait_for("document.documentElement.dataset.deckReady === 'true'", timeout)
            .await?;
        if !ready {
            diagnostics.push(
                Diagnostic::new(
                    "ready_timeout",
                    rules.severity("ready_timeout"),
                    format!("{}ms 以内に deck:ready になりませんでした", config.check.timeout_ms),
                )
                .at(slide_id, source_path),
            );
        }

        let screenshot = if options.screenshots {
            let path = screenshots_dir.join(format!("{}.png", slide_id.replace('/', "__")));
            session.screenshot(&path).await.ok().map(|()| path.to_string())
        } else {
            None
        };

        match session.evaluate::<ProbeResult>(&probe).await {
            Ok(result) => {
                let mut seen = HashSet::new();
                for probe_diagnostic in
                    result.diagnostics.into_iter().chain(result.runtime_diagnostics)
                {
                    let rule = crate::config::CheckRules::normalize(&probe_diagnostic.rule);
                    let severity = probe_diagnostic
                        .severity
                        .as_deref()
                        .and_then(parse_severity)
                        .unwrap_or_else(|| rules.severity(&rule));
                    if severity == Severity::Off {
                        continue;
                    }
                    if !seen.insert((rule.clone(), probe_diagnostic.message.clone())) {
                        continue;
                    }
                    let mut diagnostic = Diagnostic::new(rule, severity, probe_diagnostic.message)
                        .at(slide_id, source_path);
                    diagnostic.selector = probe_diagnostic.selector;
                    diagnostic.rect = probe_diagnostic.rect;
                    diagnostic.screenshot = screenshot.clone();
                    diagnostics.push(diagnostic);
                }

                if let (Some(actual), Some(slide)) = (result.step_count, manifest.slide(slide_id))
                    && actual != slide.step_count
                {
                    diagnostics.push(
                        Diagnostic::new(
                            "step_count_mismatch",
                            rules.severity("step_count_mismatch"),
                            format!(
                                "step数が静的解析と一致しません: HTML={} 実行時={actual}",
                                slide.step_count
                            ),
                        )
                        .at(slide_id, source_path),
                    );
                }
            }
            Err(error) => diagnostics.push(
                Diagnostic::new(
                    "javascript_exception",
                    rules.severity("javascript_exception"),
                    error.to_string(),
                )
                .at(slide_id, source_path),
            ),
        }

        let events = session.events();
        for exception in events.exceptions {
            diagnostics.push(
                Diagnostic::new(
                    "javascript_exception",
                    rules.severity("javascript_exception"),
                    exception,
                )
                .at(slide_id, source_path),
            );
        }
        for (url, error_text) in events.failed_requests {
            // Chromium always probes for a favicon; a deck never ships one.
            if url.ends_with("/favicon.ico") {
                continue;
            }
            diagnostics.push(
                Diagnostic::new(
                    "missing_asset",
                    rules.severity("missing_asset"),
                    format!("リソースの読み込みに失敗しました: {url} ({error_text})"),
                )
                .at(slide_id, source_path),
            );
        }
        if config.check.external_network == NetworkPolicy::Deny {
            for request in events.requests {
                if !request.starts_with(&origin)
                    && !request.starts_with("data:")
                    && !request.starts_with("about:")
                    && !request.starts_with("blob:")
                {
                    diagnostics.push(
                        Diagnostic::new(
                            "external_network",
                            rules.severity("external_network"),
                            format!("外部ネットワークへアクセスしました: {request}"),
                        )
                        .at(slide_id, source_path),
                    );
                }
            }
        }

        session.close().await;
    }

    browser.close().await;
    Ok((diagnostics, version))
}

fn parse_severity(value: &str) -> Option<Severity> {
    match value {
        "error" => Some(Severity::Error),
        "warning" => Some(Severity::Warning),
        "off" => Some(Severity::Off),
        _ => None,
    }
}

fn probe_config(project: &Project) -> String {
    let config = project.config();
    let rules: BTreeMap<&str, &str> = config
        .check
        .rules
        .as_map()
        .iter()
        .map(|(rule, severity)| (rule.as_str(), severity.as_str()))
        .collect();

    serde_json::json!({
        "canvas": { "width": config.canvas.width, "height": config.canvas.height },
        "safeArea": {
            "top": config.canvas.safe_top(),
            "right": config.canvas.safe_right(),
            "bottom": config.canvas.safe_bottom(),
            "left": config.canvas.safe_left(),
        },
        "overflowTolerancePx": config.check.overflow_tolerance_px,
        "minFontPx": config.check.min_font_px,
        "maxCharacters": config.check.max_characters,
        "ignoreSelectors": config.check.ignore.selectors,
        "rules": rules,
    })
    .to_string()
}

/* -------------------------------------------------------------------------- */
/* --changed cache                                                             */
/* -------------------------------------------------------------------------- */

fn cache_path(project: &Project) -> Utf8PathBuf {
    project.work_dir().join("cache").join("check.json")
}

fn digest(source: &str) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(source.as_bytes());
    hex::encode(hasher.finalize())
}

fn load_cache(project: &Project) -> BTreeMap<String, String> {
    std::fs::read_to_string(cache_path(project))
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_default()
}

fn record_cache(project: &Project, targets: &[SlideDocument]) -> Result<()> {
    let mut cache = load_cache(project);
    for document in targets {
        cache.insert(document.relative.clone(), digest(&document.source));
    }
    let path = cache_path(project);
    crate::error::write_file(&path, serde_json::to_string_pretty(&cache).unwrap_or_default())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn custom_element_names_are_validated() {
        assert!(is_valid_custom_element_name("deck-card"));
        assert!(is_valid_custom_element_name("rccs-metric-card"));
        assert!(!is_valid_custom_element_name("Deck-Card"));
        assert!(!is_valid_custom_element_name("card"));
        assert!(!is_valid_custom_element_name("1-card"));
    }

    #[test]
    fn collects_attribute_and_css_urls() {
        let source =
            r#"<img src="/assets/a.png"><div style="background: url('/assets/b.svg')"></div>"#;
        let urls = collect_urls(&Html::parse_document(source), source);
        assert!(urls.contains(&"/assets/a.png".to_owned()));
        assert!(urls.contains(&"/assets/b.svg".to_owned()));
    }
}
