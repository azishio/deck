//! Slide document post-processing.
//!
//! The DOM structure and meaning of a slide are never transformed (design doc
//! 19); the only edits are the `/@deck/` runtime tags, and — for a static build
//! — rewriting root-absolute URLs onto the configured base.

/// What every slide document needs in its `<head>`.
#[derive(Debug, Clone, Copy)]
pub struct RuntimeTags<'a> {
    pub base_url: &'a str,
    /// Tailwind entry, inlined as `<style type="text/tailwindcss">`. The
    /// vendored browser build compiles it inside the slide document.
    pub tailwind_css: &'a str,
}

/// Insert the runtime `<link>`/`<style>`/`<script>` tags the slide is missing,
/// so a hand-written slide works without boilerplate.
pub fn ensure_runtime_tags(html: &str, tags: &RuntimeTags<'_>) -> String {
    let base_url = tags.base_url;
    let needs_design = !html.contains("@deck/design.css");
    let needs_boot = !html.contains("@deck/boot.js");
    let needs_tailwind = !html.contains("@deck/vendor/tailwind.js");
    let needs_tailwind_input = !html.contains("text/tailwindcss");
    if !needs_design && !needs_boot && !needs_tailwind && !needs_tailwind_input {
        return html.to_owned();
    }

    let mut injection = String::new();
    if needs_design {
        injection.push_str(&format!(
            "\n  <link rel=\"stylesheet\" href=\"{base_url}@deck/design.css\">"
        ));
    }
    if needs_tailwind_input {
        // Installed from JavaScript on purpose: Chromium's HTML preload scanner
        // speculatively fetches `@import` URLs found in any inline <style>,
        // ignoring the type attribute, which would produce failed requests for
        // Tailwind's virtual stylesheets.
        injection.push_str(&format!(
            "\n  <script>\n    (() => {{\n      const style = document.createElement(\"style\");\n      style.type = \"text/tailwindcss\";\n      style.textContent = {};\n      document.head.append(style);\n    }})();\n  </script>",
            js_string_literal(tags.tailwind_css),
        ));
    }
    if needs_tailwind {
        injection
            .push_str(&format!("\n  <script src=\"{base_url}@deck/vendor/tailwind.js\"></script>"));
    }
    if needs_boot {
        injection.push_str(&format!(
            "\n  <script type=\"module\" src=\"{base_url}@deck/boot.js\"></script>"
        ));
    }
    injection.push('\n');

    match find_ci(html, "</head>") {
        Some(index) => splice(html, index, &injection),
        None => match find_ci(html, "<body") {
            Some(index) => splice(html, index, &format!("<head>{injection}</head>\n")),
            None => format!("{injection}{html}"),
        },
    }
}

/// Rewrite root-absolute URLs (`/assets/...`) onto `base_url`, and apply the
/// fingerprint map produced by `deck build`.
pub fn rewrite_urls(html: &str, base_url: &str, fingerprints: &[(String, String)]) -> String {
    let mut out = html.to_owned();
    for (from, to) in fingerprints {
        out = out.replace(from, to);
    }
    if base_url == "/" {
        return out;
    }
    for prefix in ["\"/", "'/", "(/"] {
        let quote = &prefix[..1];
        out = out.replace(prefix, &format!("{quote}{base_url}"));
    }
    // The replacement above also rewrote protocol-relative URLs; restore them.
    out.replace(&format!("{base_url}/"), "//")
}

/// JSON string literal that is also safe inside a `<script>` element.
fn js_string_literal(value: &str) -> String {
    serde_json::to_string(value).unwrap_or_else(|_| "\"\"".into()).replace("</", "<\\/")
}

fn find_ci(haystack: &str, needle: &str) -> Option<usize> {
    haystack.to_ascii_lowercase().find(&needle.to_ascii_lowercase())
}

fn splice(source: &str, index: usize, insertion: &str) -> String {
    let mut out = String::with_capacity(source.len() + insertion.len());
    out.push_str(&source[..index]);
    out.push_str(insertion);
    out.push_str(&source[index..]);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tags<'a>(base_url: &'a str, tailwind: &'a str) -> RuntimeTags<'a> {
        RuntimeTags { base_url, tailwind_css: tailwind }
    }

    #[test]
    fn injects_missing_runtime_tags() {
        let html = "<!doctype html><html><head><title>x</title></head><body></body></html>";
        let out = ensure_runtime_tags(html, &tags("/", "@import \"tailwindcss\";"));
        assert!(out.contains("/@deck/design.css"));
        assert!(out.contains("/@deck/boot.js"));
        assert!(out.contains("/@deck/vendor/tailwind.js"));
        assert!(out.contains("text/tailwindcss"));
        assert!(out.find("@deck/design.css").unwrap() < out.find("</head>").unwrap());
    }

    #[test]
    fn tailwind_input_never_reaches_the_preload_scanner() {
        let css = "@import \"tailwindcss/preflight.css\" layer(base);";
        let out = ensure_runtime_tags("<head></head>", &tags("/", css));

        // The entry must live inside a script literal, never as <style> text,
        // or Chromium would speculatively fetch the @import URLs.
        assert!(!out.contains("<style type=\"text/tailwindcss\">"));
        assert!(out.contains(&serde_json::to_string(css).unwrap()));
        let script = out.find("text/tailwindcss").unwrap();
        assert!(out[..script].contains("<script>"));
    }

    #[test]
    fn keeps_existing_runtime_tags() {
        let html = "<head><script type=\"module\" src=\"/@deck/boot.js\"></script>\
                    <script src=\"/@deck/vendor/tailwind.js\"></script>\
                    <style type=\"text/tailwindcss\"></style>\
                    <link rel=\"stylesheet\" href=\"/@deck/design.css\"></head>";
        assert_eq!(ensure_runtime_tags(html, &tags("/", "")), html);
    }

    #[test]
    fn rewrites_absolute_urls_for_a_base_url() {
        let html = "<img src=\"/assets/a.png\"><a href=\"https://example.com\">x</a>";
        let out = rewrite_urls(html, "/deck/", &[]);
        assert!(out.contains("\"/deck/assets/a.png\""));
        assert!(out.contains("https://example.com"));
    }

    #[test]
    fn applies_the_fingerprint_map() {
        let map = vec![("/assets/a.png".to_owned(), "/assets/a.abc123.png".to_owned())];
        let out = rewrite_urls("<img src=\"/assets/a.png\">", "/", &map);
        assert!(out.contains("/assets/a.abc123.png"));
    }
}
