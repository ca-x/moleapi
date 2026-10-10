use clap::{Args, Parser, Subcommand, ValueEnum};
use std::path::PathBuf;
#[derive(Parser)]
#[command(
    version,
    about = "MoleAPI collection/scenario CLI and CI runner",
    arg_required_else_help = true
)]
pub struct Cli {
    #[arg(long, global = true, conflicts_with = "database")]
    pub server: Option<String>,
    #[arg(long, global = true, conflicts_with = "server")]
    pub database: Option<PathBuf>,
    #[arg(long, global = true, conflicts_with = "token_file")]
    pub token_env: Option<String>,
    #[arg(long, global = true, conflicts_with = "token_env")]
    pub token_file: Option<PathBuf>,
    #[arg(long,global=true,default_value_t=360,value_parser=clap::value_parser!(u64).range(1..=600))]
    pub timeout: u64,
    #[command(subcommand)]
    pub command: Command,
}
#[derive(Subcommand)]
pub enum Command {
    /// Inspect machine-readable Clap command metadata.
    Schema,
    /// Run an explicitly installed official Newman CLI (optional Node runtime).
    Newman(Newman),
    /// Manage hosted personal API tokens using a login session.
    Tokens {
        #[command(subcommand)]
        command: TokenCommand,
    },
    /// Log into a remote service and save its token privately.
    Login {
        #[arg(long)]
        username: String,
        #[arg(long, default_value = "MOLEAPI_PASSWORD")]
        password_env: String,
        #[arg(long)]
        output: PathBuf,
        #[arg(long)]
        overwrite: bool,
    },
    /// List owned resource metadata without emitting saved auth/body values.
    List {
        #[arg(value_enum)]
        kind: Resource,
        #[arg(long)]
        workspace: Option<String>,
    },
    /// Import a file using shared native/Postman/OpenAPI/HAR/cURL conversion.
    Import {
        #[arg(long)]
        input: PathBuf,
        #[arg(long, default_value = "moleapi")]
        format: String,
        #[arg(long)]
        name: Option<String>,
    },
    /// Export a saved workspace using default credential screening.
    Export {
        #[arg(long)]
        workspace: String,
        #[arg(long, default_value = "moleapi")]
        format: String,
        #[arg(long)]
        output: PathBuf,
        #[arg(long)]
        overwrite: bool,
        #[arg(long)]
        include_secrets: bool,
    },
    /// Execute an owned collection or scenario with the shared runtime.
    Run(Run),
    /// Read/export a previously saved redacted report.
    Report {
        #[arg(long)]
        workspace: String,
        #[arg(long)]
        id: String,
        #[arg(long, default_value = "json")]
        format: String,
        #[arg(long, default_value = "en")]
        language: String,
        #[arg(long)]
        output: Option<PathBuf>,
        #[arg(long)]
        overwrite: bool,
    },
    /// List request-code adapters or generate one saved request snippet.
    Snippet {
        #[arg(long)]
        workspace: Option<String>,
        #[arg(long)]
        request: Option<String>,
        #[arg(long, default_value = "shell")]
        target: String,
        #[arg(long, default_value = "curl")]
        client: String,
        #[arg(long)]
        output: Option<PathBuf>,
        #[arg(long)]
        overwrite: bool,
        #[arg(long)]
        include_secrets: bool,
    },
}
#[derive(Clone, ValueEnum)]
pub enum Resource {
    Workspaces,
    Collections,
    Environments,
    Scenarios,
    Datasets,
}
#[derive(Args)]
pub struct Run {
    #[arg(long, conflicts_with = "input")]
    pub workspace: Option<String>,
    #[arg(long, conflicts_with = "workspace")]
    pub input: Option<PathBuf>,
    #[arg(long, default_value = "moleapi")]
    pub input_format: String,
    #[arg(long)]
    pub collection: Option<String>,
    #[arg(long)]
    pub scenario: Option<String>,
    /// Restrict a collection run to request IDs or unique names (repeatable).
    #[arg(long = "request", conflicts_with = "scenario")]
    pub requests: Vec<String>,
    #[arg(long)]
    pub environment: Option<String>,
    #[arg(long, conflicts_with = "data_file")]
    pub dataset: Option<String>,
    #[arg(long, conflicts_with = "dataset")]
    pub data_file: Option<PathBuf>,
    #[arg(long, default_value = "json")]
    pub data_format: String,
    #[arg(long,value_parser=clap::value_parser!(u16).range(1..=100))]
    pub iterations: Option<u16>,
    #[arg(long, conflicts_with = "no_notifications")]
    pub notify: Vec<String>,
    #[arg(long, conflicts_with = "notify")]
    pub no_notifications: bool,
    #[arg(long)]
    pub ci: bool,
    #[arg(long)]
    pub report: Option<PathBuf>,
    #[arg(long, default_value = "junit")]
    pub reporter: String,
    #[arg(long, default_value = "en")]
    pub language: String,
    #[arg(long)]
    pub overwrite: bool,
}

#[derive(Args)]
#[command(trailing_var_arg = true)]
pub struct Newman {
    #[arg(long)]
    pub node: PathBuf,
    #[arg(long)]
    pub entrypoint: PathBuf,
    /// Official Newman arguments, e.g. -- run collection.json --reporters cli,junit
    #[arg(required = true, allow_hyphen_values = true)]
    pub arguments: Vec<std::ffi::OsString>,
}

#[derive(Subcommand)]
pub enum TokenCommand {
    List,
    Create {
        #[arg(long)]
        name: String,
        #[arg(long,default_value_t=90,value_parser=clap::value_parser!(u16).range(1..=365))]
        days: u16,
        #[arg(long)]
        output: PathBuf,
        #[arg(long)]
        overwrite: bool,
    },
    Revoke {
        #[arg(long)]
        id: String,
    },
}
