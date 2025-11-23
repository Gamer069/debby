pub mod install;
pub mod view;
pub mod control;
pub mod extract;

use std::{fs, str::FromStr};

use clap::{Parser, Subcommand};
use clio::ClioPath;
use directories::ProjectDirs;
use log::{error, trace, Level};
use sqlite3::Connection;
use std::io::Write as _;

use crate::control::Control;

#[derive(Parser)]
#[command(
    name = "debby",
    version,
    about = "Installs .deb files on systems which don't support them without overcomplicating everything"
)]
struct Cli {
    #[arg(short, long, help = "Enable verbose logging (alias: v)")]
    verbose: bool,

    #[command(subcommand)]
    cmd: Commands
}

#[derive(Clone, Debug)]
pub enum UninstallInput {
    Path(ClioPath),
    PackageName(String),
    Id(usize)
}

impl FromStr for UninstallInput {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if let Ok(id) = s.parse::<usize>() {
            Ok(UninstallInput::Id(id))
        } else if s.ends_with(".deb") {
            match ClioPath::new(s) {
                Ok(path) => Ok(UninstallInput::Path(path)),
                Err(e) => Err(e.to_string()),
            }
        } else {
            Ok(UninstallInput::PackageName(s.to_string()))
        }
    }
}

#[derive(Subcommand)]
enum Commands {
    #[command(alias = "i", about = "Install a package (alias: i)")]
    Install {
        deb: ClioPath
    },

    #[command(alias = "u", about = "Uninstall a package (alias: u)")]
    Uninstall {
        // deb: ClioPath
        deb: UninstallInput
    },

    #[command(alias = "v", about = "View package info (alias: v)")]
    View {
        deb: ClioPath
    },

    #[command(alias = "c", about = "Check if package is installed or not (alias: c)")]
    Check {
        deb: ClioPath
    },

    #[command(alias = "a", about = "Fetches all installed packages (alias: a)")]
    All,
}

fn main() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info"))
        .format(|buf, record| {
            let level_color = match record.level() {
                Level::Trace => "\x1b[90m",   // Bright black / gray
                Level::Debug => "\x1b[34m",   // Blue
                Level::Info  => "\x1b[32m",   // Green
                Level::Warn  => "\x1b[33m",   // Yellow
                Level::Error => "\x1b[31m",   // Red
            };
            let reset = "\x1b[0m";

            writeln!(buf, "[{level_color}{}{reset}] ({}) {}", record.level(), record.target(), record.args())
        })
        .init();

    let cli = Cli::parse();

    let dirs = match ProjectDirs::from("me", "illia", "debby") {
        Some(dirs) => dirs,
        None => {
            error!("Failed to get project directories");
            std::process::exit(1);
        }
    };
    let db_path = dirs.data_dir().join("deb.sqlite");

    trace!("db path: {:?}", db_path);

    if let Some(parent) = db_path.parent() {
        if let Err(e) = fs::create_dir_all(parent) {
            error!("Failed to create data directory: {}", e);
            std::process::exit(1);
        }
    }

    let conn = match Connection::open(&db_path) {
        Ok(conn) => conn,
        Err(e) => {
            error!("Failed to open sqlite connection: {}", e);
            std::process::exit(1);
        }
    };

    if let Err(e) = conn.execute(
        format!(
            "CREATE TABLE IF NOT EXISTS debs (
                id INTEGER PRIMARY KEY,
                {},
                installed TEXT
            )",
            Control::sql_fields()
        )
    ) {
        error!("Failed to create table: {}", e);
        std::process::exit(1);
    }

    match cli.cmd {
        Commands::Install { deb } => {
            if let Err(e) = sudo::escalate_if_needed() {
                error!("Failed to escalate to root: {}", e);
                std::process::exit(1);
            }

            install::install(deb, dirs, conn, cli.verbose)
        },
        Commands::Uninstall { deb } => {
            if let Err(e) = sudo::escalate_if_needed() {
                error!("Failed to escalate to root: {}", e);
                std::process::exit(1);
            }

            match deb {
                UninstallInput::Path(clio_path) => {
                    install::uninstall(clio_path, dirs, conn, cli.verbose)
                },
                UninstallInput::PackageName(pkg_name) => {
                    install::uninstall_by_pkg_name(pkg_name, conn, cli.verbose);
                },
                UninstallInput::Id(id) => {
                    install::uninstall_by_id(id, conn, cli.verbose);
                },
            }
        },
        Commands::Check { deb } => {
            if let Err(e) = sudo::escalate_if_needed() {
                error!("Failed to escalate to root: {}", e);
                std::process::exit(1);
            }

            install::is_installed(deb, dirs, conn)
        },
        Commands::All => {
            if let Err(e) = sudo::escalate_if_needed() {
                error!("Failed to escalate to root: {}", e);
                std::process::exit(1);
            }

            install::all(conn)
        },
        Commands::View { deb } => view::view(deb, dirs),
    }
}
