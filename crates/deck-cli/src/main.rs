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
#[command(about = "1スライド = 1つのHTML文書のローカルスライド実行環境")]
struct Cli {
    /// deck.toml のパス
    #[arg(long, global = true)]
    config: Option<Utf8PathBuf>,

    /// プロジェクトルート
    #[arg(long, global = true)]
    root: Option<Utf8PathBuf>,

    /// 結果をJSONで出力する
    #[arg(long, global = true)]
    json: bool,

    #[arg(short, long, action = clap::ArgAction::Count, global = true)]
    verbose: u8,

    /// 色付き出力を無効にする
    #[arg(long, global = true)]
    no_color: bool,

    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// 新しいデッキを作成する
    Init(InitArgs),
    /// スライドなどを追加する
    #[command(subcommand)]
    Add(AddCommand),
    /// 開発サーバーを起動する
    Dev(DevArgs),
    /// プレゼンテーションを開始する
    Present(PresentArgs),
    /// スライドを検査する
    Check(CheckArgs),
    /// 静的配布物を生成する
    Build(BuildArgs),
    /// ページをブラウザで開く
    #[command(subcommand)]
    Open(OpenCommand),
    /// コンポーネントを操作する
    #[command(subcommand)]
    Component(ComponentCommand),
    /// 実行環境を診断する
    Doctor(DoctorArgs),
}

#[derive(Debug, Args)]
struct InitArgs {
    /// 作成先ディレクトリ (省略時はカレントディレクトリ)
    name: Option<Utf8PathBuf>,

    /// デッキタイトル
    #[arg(long)]
    title: Option<String>,

    /// テーマ
    #[arg(long, default_value = "default", value_parser = Theme::ALL)]
    theme: String,
}

#[derive(Debug, Subcommand)]
enum AddCommand {
    /// スライドを追加する
    Slide {
        /// スライド名 (ファイル名とidに使う)
        name: String,
        /// このスライドの直後に挿入する
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

    /// 起動時に開くページ
    #[arg(long, value_parser = ["none", "index", "present", "presenter", "print"])]
    open: Option<String>,

    /// 指定したスライドから開始する
    #[arg(long)]
    slide: Option<String>,

    /// Hot Reload を無効にする
    #[arg(long)]
    no_hot_reload: bool,
}

#[derive(Debug, Args)]
struct PresentArgs {
    #[command(flatten)]
    server: ServerArgs,

    /// 全画面で開く
    #[arg(long)]
    fullscreen: bool,

    /// 指定したスライドから開始する
    #[arg(long)]
    slide: Option<String>,
}

#[derive(Debug, Args)]
struct CheckArgs {
    /// 検査するスライドid (複数指定可)
    #[arg(long = "slide")]
    slides: Vec<String>,

    /// 前回から変更のあったスライドだけを検査する
    #[arg(long)]
    changed: bool,

    /// Chromium を起動せず静的検査だけ行う
    #[arg(long = "static")]
    static_only: bool,

    /// スライドごとにスクリーンショットを保存する
    #[arg(long)]
    screenshots: bool,

    /// レポート形式
    #[arg(long, default_value = "human", value_parser = ["human", "json", "sarif"])]
    report: String,

    /// レポートの出力先ファイル
    #[arg(long)]
    out: Option<Utf8PathBuf>,
}

#[derive(Debug, Args)]
struct BuildArgs {
    /// 出力先ディレクトリ
    #[arg(long)]
    out: Option<Utf8PathBuf>,

    /// 配信ベースURL ('/' で始まり '/' で終わる)
    #[arg(long)]
    base_url: Option<String>,
}

#[derive(Debug, Subcommand)]
enum OpenCommand {
    /// 印刷ページを開く (印刷そのものは実行しない)
    Print,
    /// プレゼン画面を開く
    Present,
    /// Presenter View を開く
    Presenter,
}

#[derive(Debug, Subcommand)]
enum ComponentCommand {
    /// 利用可能なコンポーネントを一覧する
    List,
    /// コンポーネントのスタイル定義を表示する
    Show { name: String },
    /// 組み込みコンポーネントのスタイルをプロジェクトへ取り出す
    Eject { name: String },
    /// 新しいコンポーネントを作成する
    New { name: String },
}

#[derive(Debug, Args)]
struct DoctorArgs {
    /// 結果をJSONで出力する
    #[arg(long)]
    json: bool,
}

fn main() -> std::process::ExitCode {
    let cli = Cli::parse();
    init_tracing(cli.verbose);

    let runtime = match tokio::runtime::Runtime::new() {
        Ok(runtime) => runtime,
        Err(error) => {
            eprintln!("error: tokio runtime を作成できません: {error}");
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
        .ok_or_else(|| Error::config("カレントディレクトリを取得できません"))?;

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

    println!("{root} に deck プロジェクトを作成しました");
    println!("\n  cd {root}\n  deck dev\n");
    Ok(())
}

fn run_add_slide(cli: &Cli, name: &str, after: Option<&str>) -> deck_core::Result<()> {
    let project = open_project(cli, Overrides::default())?;
    let path = scaffold::add_slide(&project, name, after)?;
    println!("{path} を作成しました");
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
        println!("\n警告: 0.0.0.0 へbindしています。同一ネットワークの誰でも閲覧できます。");
    }
    if hot_reload {
        server.spawn_watcher()?;
        println!("\nHot Reload: 有効");
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
            tracing::warn!("ブラウザを開けません: {error}");
        }
    }

    println!("\nCtrl-C で終了します");
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
            println!("{path} にレポートを書き出しました");
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
            "{} に {} スライド / {} アセットを出力しました (base_url = {})",
            summary.output_dir, summary.slides, summary.assets, summary.base_url
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

    println!("{url} を開きます");
    if let Err(error) = open::that_detached(&url) {
        tracing::warn!("ブラウザを開けません: {error}");
    }
    println!("Ctrl-C で終了します");
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
                return Err(Error::config(format!("スタイル定義が見つかりません: {name}")));
            }
            print!("{css}");
        }
        ComponentCommand::Eject { name } => {
            let path = scaffold::eject_component(&project, name)?;
            println!("{path} を作成しました");
            println!("deck.toml の [theme].styles へ追加してください");
        }
        ComponentCommand::New { name } => {
            let path = scaffold::new_component(&project, name)?;
            println!("{path} を作成しました");
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
        println!("\n{} スライドを検出しました", manifest.slides.len());
    }

    if report.failed() {
        return Err(Error::browser("環境に問題があります"));
    }
    Ok(())
}
