pub mod install;
pub mod view;
pub mod control;
pub mod extract;

use std::{fs, str::FromStr};

use clap::{Parser, Subcommand};
use clio::ClioPath;
use directories::ProjectDirs;
use log::{trace, Level};
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
            Ok(UninstallInput::Path(ClioPath::new(s).unwrap()))
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

    let dirs = ProjectDirs::from("me", "illia", "debby").unwrap();
    let db_path = dirs.data_dir().join("deb.sqlite");

    trace!("db path: {:?}", db_path);

    let _ = fs::create_dir_all(db_path.parent().unwrap()); // error silently

    let conn = Connection::open(&db_path).expect("Failed to open sqlite connection");

    conn.execute(
        format!(
            "CREATE TABLE IF NOT EXISTS debs (
                id INTEGER PRIMARY KEY,
                {},
                installed TEXT
            )",
            Control::sql_fields()
        )
    ).expect("Failed to create table if not exists to store installed debs");

    match cli.cmd {
        Commands::Install { deb } => {
            sudo::escalate_if_needed().expect("Failed to escalate to root");

            install::install(deb, dirs, conn)
        },
        Commands::Uninstall { deb } => {
            sudo::escalate_if_needed().expect("Failed to escalate to root");

            match deb {
                UninstallInput::Path(clio_path) => {
                    install::uninstall(clio_path, dirs, conn)
                },
                UninstallInput::PackageName(pkg_name) => {
                    install::uninstall_by_pkg_name(pkg_name, conn);
                },
                UninstallInput::Id(id) => {
                    install::uninstall_by_id(id, conn);
                },
            }
        },
        Commands::Check { deb } => {
            sudo::escalate_if_needed().expect("Failed to escalate to root");

            install::is_installed(deb, dirs, conn)
        },
        Commands::All => {
            sudo::escalate_if_needed().expect("Failed to escalate to root");

            install::all(conn)
        },
        Commands::View { deb } => view::view(deb, dirs),
    }
}
