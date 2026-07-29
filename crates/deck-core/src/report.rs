//! Check report rendering: human, JSON and SARIF (design doc 16.6).

use std::collections::BTreeMap;

use crate::check::Report;
use crate::config::Severity;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ReportFormat {
    #[default]
    Human,
    Json,
    Sarif,
}

impl ReportFormat {
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "human" | "text" => Some(Self::Human),
            "json" => Some(Self::Json),
            "sarif" => Some(Self::Sarif),
            _ => None,
        }
    }
}

pub fn render(report: &Report, format: ReportFormat, color: bool) -> String {
    match format {
        ReportFormat::Human => human(report, color),
        ReportFormat::Json => serde_json::to_string_pretty(report).unwrap_or_else(|_| "{}".into()),
        ReportFormat::Sarif => sarif(report),
    }
}

fn paint(color: bool, code: &str, text: &str) -> String {
    if color { format!("\u{1b}[{code}m{text}\u{1b}[0m") } else { text.to_owned() }
}

fn human(report: &Report, color: bool) -> String {
    let mut out = String::new();
    let mut by_slide: BTreeMap<&str, Vec<&crate::check::Diagnostic>> = BTreeMap::new();
    for diagnostic in &report.diagnostics {
        by_slide
            .entry(diagnostic.slide_id.as_deref().unwrap_or("(deck)"))
            .or_default()
            .push(diagnostic);
    }

    for (slide, diagnostics) in &by_slide {
        let path = diagnostics.first().and_then(|item| item.source_path.as_deref()).unwrap_or("");
        out.push_str(&format!("\n{}  {}\n", paint(color, "1", slide), paint(color, "2", path)));
        for diagnostic in diagnostics {
            let (code, label) = match diagnostic.severity {
                Severity::Error => ("31", "error"),
                Severity::Warning => ("33", "warning"),
                Severity::Off => continue,
            };
            out.push_str(&format!(
                "  {} {} {}\n",
                paint(color, code, label),
                paint(color, "2", &format!("[{}]", diagnostic.rule)),
                diagnostic.message,
            ));
            if let Some(selector) = &diagnostic.selector {
                out.push_str(&format!("        {}\n", paint(color, "2", selector)));
            }
            if let Some(rect) = &diagnostic.rect {
                out.push_str(&format!(
                    "        {}\n",
                    paint(
                        color,
                        "2",
                        &format!("rect {}x{} @ ({}, {})", rect.width, rect.height, rect.x, rect.y)
                    ),
                ));
            }
            if let Some(screenshot) = &diagnostic.screenshot {
                out.push_str(&format!("        {}\n", paint(color, "2", screenshot)));
            }
        }
    }

    let errors = report.errors();
    let warnings = report.warnings();
    let summary = format!(
        "\n{} slides checked: {} errors, {} warnings",
        report.slides_checked, errors, warnings
    );
    out.push_str(&paint(color, if errors > 0 { "31" } else { "32" }, &summary));
    if let Some(version) = &report.chromium_version {
        out.push_str(&paint(color, "2", &format!("\nChromium {version}")));
    }
    out.push('\n');
    out
}

fn sarif(report: &Report) -> String {
    let rules: Vec<serde_json::Value> = crate::config::BUILT_IN_RULES
        .iter()
        .map(|(rule, default_severity)| {
            serde_json::json!({
                "id": rule,
                "name": rule,
                "shortDescription": { "text": rule },
                "defaultConfiguration": { "level": sarif_level(*default_severity) },
            })
        })
        .collect();

    let results: Vec<serde_json::Value> = report
        .diagnostics
        .iter()
        .map(|diagnostic| {
            let uri = diagnostic
                .source_path
                .as_deref()
                .map(|path| format!("slides/{path}"))
                .unwrap_or_else(|| "deck.toml".to_owned());
            serde_json::json!({
                "ruleId": diagnostic.rule,
                "level": sarif_level(diagnostic.severity),
                "message": { "text": diagnostic.message },
                "locations": [{
                    "physicalLocation": {
                        "artifactLocation": { "uri": uri },
                    },
                    "logicalLocations": diagnostic.selector.as_ref().map(|selector| {
                        serde_json::json!([{ "fullyQualifiedName": selector }])
                    }),
                }],
                "properties": {
                    "slideId": diagnostic.slide_id,
                    "rect": diagnostic.rect,
                    "screenshot": diagnostic.screenshot,
                },
            })
        })
        .collect();

    let document = serde_json::json!({
        "$schema": "https://json.schemastore.org/sarif-2.1.0.json",
        "version": "2.1.0",
        "runs": [{
            "tool": {
                "driver": {
                    "name": "deck",
                    "version": env!("CARGO_PKG_VERSION"),
                    "informationUri": "https://github.com/azishio/deck",
                    "rules": rules,
                },
            },
            "results": results,
            "properties": {
                "chromiumVersion": report.chromium_version,
                "slidesChecked": report.slides_checked,
            },
        }],
    });

    serde_json::to_string_pretty(&document).unwrap_or_else(|_| "{}".into())
}

fn sarif_level(severity: Severity) -> &'static str {
    match severity {
        Severity::Error => "error",
        Severity::Warning => "warning",
        Severity::Off => "none",
    }
}
