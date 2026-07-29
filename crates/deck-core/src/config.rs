//! `deck.toml` loading and the built-in defaults.
//!
//! Merge order (design doc 6.1):
//! built-in defaults < `deck.toml` < `deck.local.toml` < environment < CLI args.

use std::collections::BTreeMap;

use camino::{Utf8Path, Utf8PathBuf};
use serde::{Deserialize, Deserializer, Serialize};

use crate::error::{Error, Result, read_to_string};

pub const CONFIG_FILE: &str = "deck.toml";
pub const LOCAL_CONFIG_FILE: &str = "deck.local.toml";
pub const LOCK_FILE: &str = "deck.lock";

/// Directory names fixed by convention (design doc 6.2).
pub const SLIDES_DIR: &str = "slides";
pub const COMPONENTS_DIR: &str = "components";
pub const DESIGN_DIR: &str = "design";
pub const ASSETS_DIR: &str = "assets";
pub const WORK_DIR: &str = ".deck";

/* -------------------------------------------------------------------------- */
/* config tree                                                                 */
/* -------------------------------------------------------------------------- */

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    #[serde(default = "default_schema")]
    pub schema: u32,
    #[serde(default)]
    pub deck: DeckMeta,
    #[serde(default)]
    pub canvas: Canvas,
    #[serde(default)]
    pub theme: Theme,
    #[serde(default)]
    pub components: ComponentsConfig,
    #[serde(default)]
    pub tailwind: TailwindConfig,
    #[serde(default)]
    pub server: ServerConfig,
    #[serde(default)]
    pub animation: AnimationConfig,
    #[serde(default)]
    pub browser: BrowserConfig,
    #[serde(default)]
    pub check: CheckConfig,
    #[serde(default)]
    pub print: PrintConfig,
    #[serde(default)]
    pub build: BuildConfig,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            schema: default_schema(),
            deck: DeckMeta::default(),
            canvas: Canvas::default(),
            theme: Theme::default(),
            components: ComponentsConfig::default(),
            tailwind: TailwindConfig::default(),
            server: ServerConfig::default(),
            animation: AnimationConfig::default(),
            browser: BrowserConfig::default(),
            check: CheckConfig::default(),
            print: PrintConfig::default(),
            build: BuildConfig::default(),
        }
    }
}

const fn default_schema() -> u32 {
    1
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeckMeta {
    #[serde(default = "default_title")]
    pub title: String,
    #[serde(default)]
    pub author: String,
    #[serde(default = "default_lang")]
    pub lang: String,
}

impl Default for DeckMeta {
    fn default() -> Self {
        Self { title: default_title(), author: String::new(), lang: default_lang() }
    }
}

fn default_title() -> String {
    "Deck".into()
}

fn default_lang() -> String {
    "en".into()
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Canvas {
    #[serde(default = "default_canvas_width")]
    pub width: u32,
    #[serde(default = "default_canvas_height")]
    pub height: u32,
    /// `[top, right, bottom, left]`
    #[serde(default = "default_safe_area")]
    pub safe_area: [u32; 4],
}

impl Default for Canvas {
    fn default() -> Self {
        Self {
            width: default_canvas_width(),
            height: default_canvas_height(),
            safe_area: default_safe_area(),
        }
    }
}

impl Canvas {
    pub fn safe_top(&self) -> u32 {
        self.safe_area[0]
    }
    pub fn safe_right(&self) -> u32 {
        self.safe_area[1]
    }
    pub fn safe_bottom(&self) -> u32 {
        self.safe_area[2]
    }
    pub fn safe_left(&self) -> u32 {
        self.safe_area[3]
    }
}

const fn default_canvas_width() -> u32 {
    1280
}
const fn default_canvas_height() -> u32 {
    720
}
const fn default_safe_area() -> [u32; 4] {
    [56, 64, 56, 64]
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Theme {
    /// Project stylesheets merged into `/@deck/design.css`, in order.
    #[serde(default = "default_theme_styles")]
    pub styles: Vec<Utf8PathBuf>,
}

impl Default for Theme {
    fn default() -> Self {
        Self { styles: default_theme_styles() }
    }
}

fn default_theme_styles() -> Vec<Utf8PathBuf> {
    ["design/tokens.css", "design/theme.css", "design/overrides.css"]
        .into_iter()
        .map(Utf8PathBuf::from)
        .collect()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComponentsConfig {
    #[serde(default = "default_components_entry")]
    pub entry: Utf8PathBuf,
}

impl Default for ComponentsConfig {
    fn default() -> Self {
        Self { entry: default_components_entry() }
    }
}

fn default_components_entry() -> Utf8PathBuf {
    Utf8PathBuf::from("components/index.js")
}

/// Tailwind CSS is a required part of the slide runtime: the vendored browser
/// build compiles utilities inside every slide document, so no Node.js
/// toolchain is involved.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TailwindConfig {
    /// Project stylesheet used as the Tailwind entry (`@theme`, `@utility`,
    /// `@apply` live here).
    #[serde(default = "default_tailwind_entry")]
    pub entry: Utf8PathBuf,
    /// Include Tailwind's preflight reset at the head of the entry. It lands in
    /// `@layer base`, which sits below the deck design system, so deck's own
    /// `@layer deck.reset` and the component styles still win.
    #[serde(default = "default_true")]
    pub preflight: bool,
}

impl Default for TailwindConfig {
    fn default() -> Self {
        Self { entry: default_tailwind_entry(), preflight: true }
    }
}

fn default_tailwind_entry() -> Utf8PathBuf {
    Utf8PathBuf::from("design/tailwind.css")
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    #[serde(default = "default_host")]
    pub host: String,
    /// `0` asks the OS for an ephemeral port.
    #[serde(default)]
    pub port: u16,
    #[serde(default)]
    pub open: OpenTarget,
    #[serde(default = "default_true")]
    pub hot_reload: bool,
    #[serde(default = "default_preload")]
    pub preload: u8,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            host: default_host(),
            port: 0,
            open: OpenTarget::default(),
            hot_reload: true,
            preload: default_preload(),
        }
    }
}

fn default_host() -> String {
    "127.0.0.1".into()
}
const fn default_true() -> bool {
    true
}
const fn default_preload() -> u8 {
    1
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OpenTarget {
    None,
    Index,
    Present,
    #[default]
    Presenter,
    Print,
}

impl OpenTarget {
    pub fn path(self) -> Option<&'static str> {
        match self {
            Self::None => None,
            Self::Index => Some("/"),
            Self::Present => Some("/present"),
            Self::Presenter => Some("/presenter"),
            Self::Print => Some("/print"),
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AnimationConfig {
    #[serde(default)]
    pub engine: AnimationEngine,
    #[serde(default)]
    pub reduced_motion: ReducedMotion,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AnimationEngine {
    #[default]
    Animejs,
    None,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ReducedMotion {
    /// Honour the OS setting and jump straight to the final state.
    #[default]
    Instant,
    /// Honour the OS setting.
    Respect,
    /// Always animate.
    Ignore,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BrowserConfig {
    #[serde(default = "default_browser_command")]
    pub command: String,
    #[serde(default = "default_true")]
    pub headless: bool,
    /// Chromium's setuid/namespace sandbox. Container images and CI runners
    /// often restrict unprivileged user namespaces, which makes Chromium abort
    /// on start; turning this off is the documented workaround.
    #[serde(default = "default_true")]
    pub sandbox: bool,
}

impl Default for BrowserConfig {
    fn default() -> Self {
        Self { command: default_browser_command(), headless: true, sandbox: true }
    }
}

fn default_browser_command() -> String {
    "chromium".into()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckConfig {
    #[serde(default)]
    pub on_save: OnSave,
    #[serde(default = "default_timeout_ms")]
    pub timeout_ms: u64,
    #[serde(default = "default_min_font_px")]
    pub min_font_px: f64,
    #[serde(default = "default_overflow_tolerance")]
    pub overflow_tolerance_px: f64,
    #[serde(default = "default_max_characters")]
    pub max_characters: u32,
    #[serde(default)]
    pub external_network: NetworkPolicy,
    #[serde(default)]
    pub rules: CheckRules,
    #[serde(default)]
    pub ignore: CheckIgnore,
}

impl Default for CheckConfig {
    fn default() -> Self {
        Self {
            on_save: OnSave::default(),
            timeout_ms: default_timeout_ms(),
            min_font_px: default_min_font_px(),
            overflow_tolerance_px: default_overflow_tolerance(),
            max_characters: default_max_characters(),
            external_network: NetworkPolicy::default(),
            rules: CheckRules::default(),
            ignore: CheckIgnore::default(),
        }
    }
}

const fn default_timeout_ms() -> u64 {
    10_000
}
const fn default_min_font_px() -> f64 {
    18.0
}
const fn default_overflow_tolerance() -> f64 {
    1.0
}
const fn default_max_characters() -> u32 {
    900
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OnSave {
    Off,
    #[default]
    Changed,
    All,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum NetworkPolicy {
    #[default]
    Deny,
    Allow,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CheckIgnore {
    #[serde(default)]
    pub selectors: Vec<String>,
    /// Slide ids excluded from checking entirely.
    #[serde(default)]
    pub slides: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    #[default]
    Error,
    Warning,
    Off,
}

impl Severity {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Error => "error",
            Self::Warning => "warning",
            Self::Off => "off",
        }
    }
}

/// Per-rule severities. Unknown keys are rejected so typos do not silently
/// disable a check.
#[derive(Debug, Clone, Serialize)]
pub struct CheckRules(BTreeMap<String, Severity>);

/// `(rule, default severity)` for every rule deck knows about.
pub const BUILT_IN_RULES: &[(&str, Severity)] = &[
    // static
    ("duplicate_slide_id", Severity::Error),
    ("missing_title", Severity::Warning),
    ("missing_deck_slide", Severity::Error),
    ("duplicate_html_id", Severity::Error),
    ("invalid_component_name", Severity::Error),
    ("invalid_local_url", Severity::Error),
    ("missing_file", Severity::Error),
    ("external_url", Severity::Error),
    // runtime
    ("console_error", Severity::Error),
    ("javascript_exception", Severity::Error),
    ("unhandled_rejection", Severity::Error),
    ("missing_asset", Severity::Error),
    ("missing_font", Severity::Error),
    ("undefined_component", Severity::Error),
    ("external_network", Severity::Error),
    ("ready_timeout", Severity::Error),
    ("step_count_mismatch", Severity::Warning),
    ("animation_engine", Severity::Warning),
    // layout
    ("slide_overflow", Severity::Error),
    ("clipped_text", Severity::Error),
    ("outside_canvas", Severity::Error),
    ("outside_safe_area", Severity::Warning),
    ("text_overlap", Severity::Warning),
    ("min_font_size", Severity::Warning),
    ("low_contrast", Severity::Warning),
    ("text_density", Severity::Warning),
];

impl Default for CheckRules {
    fn default() -> Self {
        Self(
            BUILT_IN_RULES.iter().map(|(name, severity)| ((*name).to_owned(), *severity)).collect(),
        )
    }
}

impl CheckRules {
    /// Rule names use `snake_case`; the browser side reports `kebab-case`.
    pub fn normalize(rule: &str) -> String {
        rule.replace('-', "_")
    }

    pub fn severity(&self, rule: &str) -> Severity {
        self.0.get(&Self::normalize(rule)).copied().unwrap_or(Severity::Error)
    }

    pub fn as_map(&self) -> &BTreeMap<String, Severity> {
        &self.0
    }
}

impl<'de> Deserialize<'de> for CheckRules {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        use serde::de::Error as _;

        let overrides = BTreeMap::<String, Severity>::deserialize(deserializer)?;
        let mut rules = Self::default();
        for (rule, severity) in overrides {
            let key = Self::normalize(&rule);
            if !rules.0.contains_key(&key) {
                return Err(D::Error::custom(format!("unknown check rule: {rule}")));
            }
            rules.0.insert(key, severity);
        }
        Ok(rules)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrintConfig {
    #[serde(default = "default_print_route")]
    pub route: String,
    #[serde(default)]
    pub steps: PrintSteps,
    #[serde(default = "default_true")]
    pub preflight: bool,
    #[serde(default)]
    pub show_notes: bool,
}

impl Default for PrintConfig {
    fn default() -> Self {
        Self {
            route: default_print_route(),
            steps: PrintSteps::default(),
            preflight: true,
            show_notes: false,
        }
    }
}

fn default_print_route() -> String {
    "/print".into()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PrintSteps {
    #[default]
    Final,
    Initial,
    Each,
}

impl PrintSteps {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Final => "final",
            Self::Initial => "initial",
            Self::Each => "each",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BuildConfig {
    #[serde(default = "default_output_dir")]
    pub output_dir: Utf8PathBuf,
    #[serde(default = "default_base_url")]
    pub base_url: String,
    #[serde(default = "default_true")]
    pub fingerprint_assets: bool,
}

impl Default for BuildConfig {
    fn default() -> Self {
        Self {
            output_dir: default_output_dir(),
            base_url: default_base_url(),
            fingerprint_assets: true,
        }
    }
}

fn default_output_dir() -> Utf8PathBuf {
    Utf8PathBuf::from("dist")
}
fn default_base_url() -> String {
    "/".into()
}

/* -------------------------------------------------------------------------- */
/* loading                                                                     */
/* -------------------------------------------------------------------------- */

/// Values supplied on the command line. Applied last (design doc 6.1).
#[derive(Debug, Clone, Default)]
pub struct Overrides {
    pub host: Option<String>,
    pub port: Option<u16>,
    pub open: Option<OpenTarget>,
    pub hot_reload: Option<bool>,
    pub browser_command: Option<String>,
    pub headless: Option<bool>,
    pub base_url: Option<String>,
    pub output_dir: Option<Utf8PathBuf>,
}

impl Config {
    /// Load, merging `deck.toml`, `deck.local.toml`, environment and CLI values.
    pub fn load(
        root: &Utf8Path,
        config_path: Option<&Utf8Path>,
        overrides: &Overrides,
    ) -> Result<Self> {
        let primary = config_path.map_or_else(|| root.join(CONFIG_FILE), Utf8Path::to_path_buf);
        let mut merged = toml::Value::Table(toml::map::Map::new());

        for path in [primary, root.join(LOCAL_CONFIG_FILE)] {
            if !path.is_file() {
                continue;
            }
            let text = read_to_string(&path)?;
            let value: toml::Value =
                toml::from_str(&text).map_err(|error| Error::config(format!("{path}: {error}")))?;
            merge_toml(&mut merged, value);
        }

        apply_env(&mut merged);

        let mut config: Self =
            merged.try_into().map_err(|error| Error::config(format!("{CONFIG_FILE}: {error}")))?;

        if config.schema != 1 {
            return Err(Error::config(format!(
                "unsupported schema = {} (this build only understands schema = 1)",
                config.schema
            )));
        }

        config.apply(overrides);
        config.validate()?;
        Ok(config)
    }

    fn apply(&mut self, overrides: &Overrides) {
        if let Some(host) = &overrides.host {
            self.server.host = host.clone();
        }
        if let Some(port) = overrides.port {
            self.server.port = port;
        }
        if let Some(open) = overrides.open {
            self.server.open = open;
        }
        if let Some(hot_reload) = overrides.hot_reload {
            self.server.hot_reload = hot_reload;
        }
        if let Some(command) = &overrides.browser_command {
            self.browser.command = command.clone();
        }
        if let Some(headless) = overrides.headless {
            self.browser.headless = headless;
        }
        if let Some(base_url) = &overrides.base_url {
            self.build.base_url = base_url.clone();
        }
        if let Some(output_dir) = &overrides.output_dir {
            self.build.output_dir = output_dir.clone();
        }
    }

    fn validate(&self) -> Result<()> {
        if self.canvas.width == 0 || self.canvas.height == 0 {
            return Err(Error::config("[canvas] width and height must be at least 1"));
        }
        if !self.build.base_url.starts_with('/') || !self.build.base_url.ends_with('/') {
            return Err(Error::config(format!(
                "[build] base_url must start and end with '/': {}",
                self.build.base_url
            )));
        }
        Ok(())
    }
}

fn merge_toml(target: &mut toml::Value, source: toml::Value) {
    match (target, source) {
        (toml::Value::Table(target), toml::Value::Table(source)) => {
            for (key, value) in source {
                match target.get_mut(&key) {
                    Some(existing) => merge_toml(existing, value),
                    None => {
                        target.insert(key, value);
                    }
                }
            }
        }
        (target, source) => *target = source,
    }
}

/// `DECK_*` environment overrides, applied between the TOML files and CLI args.
fn apply_env(value: &mut toml::Value) {
    const STRING_VARS: &[(&str, &[&str])] = &[
        ("DECK_TITLE", &["deck", "title"]),
        ("DECK_LANG", &["deck", "lang"]),
        ("DECK_HOST", &["server", "host"]),
        ("DECK_OPEN", &["server", "open"]),
        ("DECK_BROWSER", &["browser", "command"]),
        ("DECK_BASE_URL", &["build", "base_url"]),
        ("DECK_OUTPUT_DIR", &["build", "output_dir"]),
        ("DECK_EXTERNAL_NETWORK", &["check", "external_network"]),
        ("DECK_PRINT_STEPS", &["print", "steps"]),
    ];
    const INT_VARS: &[(&str, &[&str])] = &[
        ("DECK_PORT", &["server", "port"]),
        ("DECK_PRELOAD", &["server", "preload"]),
        ("DECK_CHECK_TIMEOUT_MS", &["check", "timeout_ms"]),
        ("DECK_CANVAS_WIDTH", &["canvas", "width"]),
        ("DECK_CANVAS_HEIGHT", &["canvas", "height"]),
    ];
    const BOOL_VARS: &[(&str, &[&str])] = &[
        ("DECK_HOT_RELOAD", &["server", "hot_reload"]),
        ("DECK_HEADLESS", &["browser", "headless"]),
        ("DECK_FINGERPRINT_ASSETS", &["build", "fingerprint_assets"]),
        ("DECK_TAILWIND_PREFLIGHT", &["tailwind", "preflight"]),
        ("DECK_BROWSER_SANDBOX", &["browser", "sandbox"]),
    ];

    for (variable, path) in STRING_VARS {
        if let Ok(raw) = std::env::var(variable) {
            set_path(value, path, toml::Value::String(raw));
        }
    }
    for (variable, path) in INT_VARS {
        if let Ok(parsed) =
            std::env::var(variable).map_err(drop).and_then(|raw| raw.parse::<i64>().map_err(drop))
        {
            set_path(value, path, toml::Value::Integer(parsed));
        }
    }
    for (variable, path) in BOOL_VARS {
        if let Ok(raw) = std::env::var(variable) {
            let parsed = matches!(raw.as_str(), "1" | "true" | "yes" | "on");
            set_path(value, path, toml::Value::Boolean(parsed));
        }
    }
}

fn set_path(value: &mut toml::Value, path: &[&str], leaf: toml::Value) {
    let Some((key, rest)) = path.split_first() else {
        *value = leaf;
        return;
    };
    if !value.is_table() {
        *value = toml::Value::Table(toml::map::Map::new());
    }
    let table = value.as_table_mut().expect("table");
    if rest.is_empty() {
        table.insert((*key).to_owned(), leaf);
        return;
    }
    let entry =
        table.entry((*key).to_owned()).or_insert_with(|| toml::Value::Table(toml::map::Map::new()));
    set_path(entry, rest, leaf);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_match_the_design_document() {
        let config = Config::default();
        assert_eq!(config.canvas.width, 1280);
        assert_eq!(config.canvas.height, 720);
        assert_eq!(config.canvas.safe_area, [56, 64, 56, 64]);
        assert_eq!(config.server.preload, 1);
        assert_eq!(config.check.rules.severity("slide_overflow"), Severity::Error);
        assert_eq!(config.check.rules.severity("outside_safe_area"), Severity::Warning);
    }

    #[test]
    fn kebab_case_rule_names_resolve() {
        let rules = CheckRules::default();
        assert_eq!(rules.severity("outside-safe-area"), Severity::Warning);
    }

    #[test]
    fn unknown_rule_is_rejected() {
        let error = toml::from_str::<CheckConfig>("[rules]\nnope = \"error\"\n").unwrap_err();
        assert!(error.to_string().contains("unknown check rule"));
    }

    #[test]
    fn local_config_overrides_primary() {
        let mut merged =
            toml::from_str::<toml::Value>("[server]\nport = 1\nhost = \"a\"\n").unwrap();
        merge_toml(&mut merged, toml::from_str("[server]\nport = 2\n").unwrap());
        let config: Config = merged.try_into().unwrap();
        assert_eq!(config.server.port, 2);
        assert_eq!(config.server.host, "a");
    }
}
