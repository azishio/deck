//! `deck` command line interface (design doc 17).

use camino::{Utf8Path, Utf8PathBuf};
use clap::{Args, Parser, Subcommand};
use deck_core::check::CheckOptions;
use deck_core::config::{OpenTarget, Overrides};
use deck_core::error::Error;
use deck_core::project::Project;
use deck_core::report::ReportFormat;
use deck_core::scaffold::{self, Theme};
use deck_core::server::Server;
use deck_core::{build, check, doctor, lock, manifest, report};

#[derive(Debug, Parser)]
#[command(name = "deck")]
#[command(version)]
#[command(propagate_version = true)]
#[command(about = "A local slide runtime where one slide is one complete HTML document")]
struct Cli {
    /// Path to deck.toml
    #[arg(long, global = true)]
    config: Option<Utf8PathBuf>,

    /// Project root
    #[arg(long, global = true)]
    root: Option<Utf8PathBuf>,

    /// Print the result as JSON
    #[arg(long, global = true)]
    json: bool,

    #[arg(short, long, action = clap::ArgAction::Count, global = true)]
    verbose: u8,

    /// Disable coloured output
    #[arg(long, global = true)]
    no_color: bool,

    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Create a new deck
    Init(InitArgs),
    /// Add a slide
    #[command(subcommand)]
    Add(AddCommand),
    /// Start the development server
    Dev(DevArgs),
    /// Start presenting
    Present(PresentArgs),
    /// Check the slides
    Check(CheckArgs),
    /// Produce a static build
    Build(BuildArgs),
    /// Open a page in the browser
    #[command(subcommand)]
    Open(OpenCommand),
    /// Work with components
    #[command(subcommand)]
    Component(ComponentCommand),
    /// Diagnose the environment
    Doctor(DoctorArgs),
}

#[derive(Debug, Args)]
struct InitArgs {
    /// Target directory (defaults to the current one)
    name: Option<Utf8PathBuf>,

    /// Deck title
    #[arg(long)]
    title: Option<String>,

    /// Theme
    #[arg(long, default_value = "default", value_parser = Theme::ALL)]
    theme: String,
}

#[derive(Debug, Subcommand)]
enum AddCommand {
    /// Add a slide
    Slide {
        /// Slide name, used for the file name and the id
        name: String,
        /// Insert directly after this slide
        #[arg(long)]
        after: Option<String>,
    },
}

#[derive(Debug, Args)]
struct ServerArgs {
    #[arg(long)]
    host: Option<String>,
    #[arg(long)]
    port: Option<u16>,
}

#[derive(Debug, Args)]
struct DevArgs {
    #[command(flatten)]
    server: ServerArgs,

    /// Page to open on startup
    #[arg(long, value_parser = ["none", "index", "present", "presenter", "print"])]
    open: Option<String>,

    /// Start from this slide
    #[arg(long)]
    slide: Option<String>,

    /// Disable hot reload
    #[arg(long)]
    no_hot_reload: bool,
}

#[derive(Debug, Args)]
struct PresentArgs {
    #[command(flatten)]
    server: ServerArgs,

    /// Open in fullscreen
    #[arg(long)]
    fullscreen: bool,

    /// Start from this slide
    #[arg(long)]
    slide: Option<String>,
}

#[derive(Debug, Args)]
struct CheckArgs {
    /// Slide id to check; repeatable
    #[arg(long = "slide")]
    slides: Vec<String>,

    /// Only check slides that changed since the last run
    #[arg(long)]
    changed: bool,

    /// Only run the static checks, without launching Chromium
    #[arg(long = "static")]
    static_only: bool,

    /// Save a screenshot per slide
    #[arg(long)]
    screenshots: bool,

    /// Report format
    #[arg(long, default_value = "human", value_parser = ["human", "json", "sarif"])]
    report: String,

    /// Write the report to this file
    #[arg(long)]
    out: Option<Utf8PathBuf>,
}

#[derive(Debug, Args)]
struct BuildArgs {
    /// Output directory
    #[arg(long)]
    out: Option<Utf8PathBuf>,

    /// Base URL to serve from; must start and end with '/'
    #[arg(long)]
    base_url: Option<String>,
}

#[derive(Debug, Subcommand)]
enum OpenCommand {
    /// Open the print page; printing itself stays with the browser
    Print,
    /// Open the audience view
    Present,
    /// Open the presenter view
    Presenter,
}

#[derive(Debug, Subcommand)]
enum ComponentCommand {
    /// List the available components
    List,
    /// Print a component's style definition
    Show { name: String },
    /// Copy a built-in component's styles into the project
    Eject { name: String },
    /// Scaffold a new component
    New { name: String },
}

#[derive(Debug, Args)]
struct DoctorArgs {
    /// Print the result as JSON
    #[arg(long)]
    json: bool,
}

fn main() -> std::process::ExitCode {
    let cli = Cli::parse();
    init_tracing(cli.verbose);

    let runtime = match tokio::runtime::Runtime::new() {
        Ok(runtime) => runtime,
        Err(error) => {
            eprintln!("error: could not create the tokio runtime: {error}");
            return std::process::ExitCode::from(2);
        }
    };

    match runtime.block_on(dispatch(&cli)) {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("error: {error}");
            std::process::ExitCode::from(u8::try_from(error.exit_code()).unwrap_or(2))
        }
    }
}

fn init_tracing(verbose: u8) {
    let level = match verbose {
        0 => "warn",
        1 => "info",
        2 => "debug",
        _ => "trace",
    };
    let filter = tracing_subscriber::EnvFilter::try_from_env("DECK_LOG").unwrap_or_else(|_| {
        tracing_subscriber::EnvFilter::new(format!("deck_core={level},deck_cli={level}"))
    });
    tracing_subscriber::fmt().with_env_filter(filter).with_target(false).without_time().init();
}

async fn dispatch(cli: &Cli) -> deck_core::Result<()> {
    match &cli.command {
        Command::Init(args) => run_init(cli, args),
        Command::Add(AddCommand::Slide { name, after }) => {
            run_add_slide(cli, name, after.as_deref())
        }
        Command::Dev(args) => run_dev(cli, args).await,
        Command::Present(args) => run_present(cli, args).await,
        Command::Check(args) => run_check(cli, args).await,
        Command::Build(args) => run_build(cli, args),
        Command::Open(command) => run_open(cli, command).await,
        Command::Component(command) => run_component(cli, command),
        Command::Doctor(args) => run_doctor(cli, args).await,
    }
}

fn open_project(cli: &Cli, overrides: Overrides) -> deck_core::Result<Project> {
    Project::open(cli.root.as_deref(), cli.config.as_deref(), &overrides)
}

fn use_color(cli: &Cli) -> bool {
    !cli.no_color && std::env::var_os("NO_COLOR").is_none()
}

/* -------------------------------------------------------------------------- */
/* commands                                                                    */
/* -------------------------------------------------------------------------- */

fn run_init(cli: &Cli, args: &InitArgs) -> deck_core::Result<()> {
    let cwd = std::env::current_dir()
        .ok()
        .and_then(|path| Utf8PathBuf::from_path_buf(path).ok())
        .ok_or_else(|| Error::config("could not read the current directory"))?;

    let root = match (&cli.root, &args.name) {
        (Some(root), _) => root.clone(),
        (None, Some(name)) => cwd.join(name),
        (None, None) => cwd,
    };
    let title = args
        .title
        .clone()
        .or_else(|| root.file_name().map(str::to_owned))
        .unwrap_or_else(|| "Deck".to_owned());
    let theme = Theme::parse(&args.theme).unwrap_or_default();

    scaffold::init(&root, &title, theme)?;

    println!("Created a deck project in {root}");
    println!("\n  cd {root}\n  deck dev\n");
    Ok(())
}

fn run_add_slide(cli: &Cli, name: &str, after: Option<&str>) -> deck_core::Result<()> {
    let project = open_project(cli, Overrides::default())?;
    let path = scaffold::add_slide(&project, name, after)?;
    println!("Created {path}");
    Ok(())
}

async fn run_dev(cli: &Cli, args: &DevArgs) -> deck_core::Result<()> {
    let overrides = Overrides {
        host: args.server.host.clone(),
        port: args.server.port,
        open: args.open.as_deref().and_then(parse_open_target),
        hot_reload: args.no_hot_reload.then_some(false),
        ..Overrides::default()
    };
    let project = open_project(cli, overrides)?;
    serve(project, args.slide.as_deref(), false).await
}

async fn run_present(cli: &Cli, args: &PresentArgs) -> deck_core::Result<()> {
    let overrides = Overrides {
        host: args.server.host.clone(),
        port: args.server.port,
        open: Some(OpenTarget::Present),
        ..Overrides::default()
    };
    let project = open_project(cli, overrides)?;
    serve(project, args.slide.as_deref(), args.fullscreen).await
}

async fn serve(project: Project, slide: Option<&str>, fullscreen: bool) -> deck_core::Result<()> {
    let hot_reload = project.config().server.hot_reload;
    let open_target = project.config().server.open;
    let host = project.config().server.host.clone();

    let server = Server::bind(project).await?;
    let origin = server.origin();

    println!("deck: {origin}");
    println!("  present   {origin}/present");
    println!("  presenter {origin}/presenter");
    println!("  print     {origin}/print");
    if host == "0.0.0.0" {
        println!("\nWarning: bound to 0.0.0.0 — anyone on this network can view the deck.");
    }
    if hot_reload {
        server.spawn_watcher()?;
        println!("\nHot reload: on");
    }

    if let Some(path) = open_target.path() {
        let mut url = format!("{origin}{path}");
        if fullscreen {
            url.push_str("?fullscreen=1");
        }
        if let Some(slide) = slide {
            url.push_str(&format!("#/{slide}/0"));
        }
        if let Err(error) = open::that_detached(&url) {
            tracing::warn!("could not open a browser: {error}");
        }
    }

    println!("\nPress Ctrl-C to stop");
    server.serve().await
}

fn parse_open_target(value: &str) -> Option<OpenTarget> {
    match value {
        "none" => Some(OpenTarget::None),
        "index" => Some(OpenTarget::Index),
        "present" => Some(OpenTarget::Present),
        "presenter" => Some(OpenTarget::Presenter),
        "print" => Some(OpenTarget::Print),
        _ => None,
    }
}

async fn run_check(cli: &Cli, args: &CheckArgs) -> deck_core::Result<()> {
    let project = open_project(cli, Overrides::default())?;
    let options = CheckOptions {
        slides: args.slides.clone(),
        changed_only: args.changed,
        static_only: args.static_only,
        screenshots: args.screenshots,
    };

    let report = check::run(&project, &options).await?;
    let format = if cli.json {
        ReportFormat::Json
    } else {
        ReportFormat::parse(&args.report).unwrap_or_default()
    };
    let rendered = report::render(&report, format, use_color(cli) && format == ReportFormat::Human);

    match &args.out {
        Some(path) => {
            let path = resolve_output(project.root(), path);
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent).map_err(|error| Error::io(parent, error))?;
            }
            std::fs::write(&path, &rendered).map_err(|error| Error::io(&path, error))?;
            println!("Wrote the report to {path}");
        }
        None => print!("{rendered}"),
    }

    report.into_result().map(drop)
}

fn resolve_output(root: &Utf8Path, path: &Utf8Path) -> Utf8PathBuf {
    if path.is_absolute() { path.to_path_buf() } else { root.join(path) }
}

fn run_build(cli: &Cli, args: &BuildArgs) -> deck_core::Result<()> {
    let overrides = Overrides {
        base_url: args.base_url.clone(),
        output_dir: args.out.clone(),
        ..Overrides::default()
    };
    let project = open_project(cli, overrides)?;
    let summary = build::run(&project)?;

    let mut lockfile = lock::Lock::load(project.root())?.unwrap_or_default();
    lockfile.deck_runtime = env!("CARGO_PKG_VERSION").to_owned();
    lockfile.animejs = lock::ANIMEJS_VERSION.to_owned();
    lockfile.tailwindcss = lock::TAILWIND_VERSION.to_owned();
    lockfile.components = lock::COMPONENTS_VERSION.to_owned();
    lockfile.theme = lock::THEME_VERSION.to_owned();
    lockfile.save(project.root())?;

    if cli.json {
        println!("{}", serde_json::to_string_pretty(&summary).unwrap_or_default());
    } else {
        println!(
            "Wrote {} slides and {} assets to {} (base_url = {})",
            summary.slides, summary.assets, summary.output_dir, summary.base_url
        );
    }
    Ok(())
}

async fn run_open(cli: &Cli, command: &OpenCommand) -> deck_core::Result<()> {
    let path = match command {
        OpenCommand::Print => "/print",
        OpenCommand::Present => "/present",
        OpenCommand::Presenter => "/presenter",
    };
    let overrides = Overrides { open: Some(OpenTarget::None), ..Overrides::default() };
    let project = open_project(cli, overrides)?;

    let server = Server::bind(project).await?;
    let url = format!("{}{path}", server.origin());
    server.spawn_watcher().ok();

    println!("Opening {url}");
    if let Err(error) = open::that_detached(&url) {
        tracing::warn!("could not open a browser: {error}");
    }
    println!("Press Ctrl-C to stop");
    server.serve().await
}

fn run_component(cli: &Cli, command: &ComponentCommand) -> deck_core::Result<()> {
    let project = open_project(cli, Overrides::default())?;

    match command {
        ComponentCommand::List => {
            let components = scaffold::list_components(&project);
            if cli.json {
                let value: Vec<_> = components
                    .iter()
                    .map(|component| {
                        serde_json::json!({
                            "name": component.name,
                            "builtIn": component.built_in,
                            "source": component.source,
                        })
                    })
                    .collect();
                println!("{}", serde_json::to_string_pretty(&value).unwrap_or_default());
            } else {
                for component in components {
                    let origin = component.source.unwrap_or_else(|| "built-in".to_owned());
                    println!("{:<24} {origin}", component.name);
                }
            }
        }
        ComponentCommand::Show { name } => {
            let css = scaffold::component_css(name);
            if css.is_empty() {
                return Err(Error::config(format!("no style definition found for {name}")));
            }
            print!("{css}");
        }
        ComponentCommand::Eject { name } => {
            let path = scaffold::eject_component(&project, name)?;
            println!("Created {path}");
            println!("Add it to [theme].styles in deck.toml to take effect");
        }
        ComponentCommand::New { name } => {
            let path = scaffold::new_component(&project, name)?;
            println!("Created {path}");
        }
    }
    Ok(())
}

async fn run_doctor(cli: &Cli, args: &DoctorArgs) -> deck_core::Result<()> {
    let project = open_project(cli, Overrides::default())?;
    let report = doctor::run(&project).await?;

    if cli.json || args.json {
        println!("{}", serde_json::to_string_pretty(&report).unwrap_or_default());
    } else {
        println!("{}", report.to_text());
        let manifest = manifest::Manifest::build(&project.slides_dir(), 1)?;
        println!("\n{} slides discovered", manifest.slides.len());
    }

    if report.failed() {
        return Err(Error::browser("the environment has problems"));
    }
    Ok(())
}
