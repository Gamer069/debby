use std::{error, fs::File, path::{Path, PathBuf}};

use clio::ClioPath;
use colored::Colorize;
use directories::ProjectDirs;
use log::{error, info, warn};
use sqlite3::Connection;
use walkdir::WalkDir;

use crate::{control::{self, ControlWithData}, extract};

pub fn install(deb: ClioPath, dirs: ProjectDirs, conn: Connection) {
    if !deb.exists() {
        error!("Failed to install .deb file because the .deb file you specified does not exist.");
        std::process::exit(-1);
    }

    if deb.extension().is_none_or(|ext| ext != "deb") {
        error!("Failed to install .deb file because the file you specified isn't one.");
        std::process::exit(-1);
    }

    let f = File::open(deb.to_path_buf()).unwrap();

    let cache_dir = dirs.cache_dir();
    let extract_dir = cache_dir.join("extracted");

    let _ = std::fs::remove_dir_all(&extract_dir);

    extract::extract_to(extract_dir.clone(), f);

    let ctrl_path = extract_dir.join("control").join("control");

    if !ctrl_path.is_file() {
        error!("Failed to get control file from .deb, make sure the .deb is valid");
        std::process::exit(-1);
    }

    let installed = copy(extract_dir);

    let ctrl_str = std::fs::read_to_string(ctrl_path).expect("Failed to read control file");
    let ctrl = control::parse_control(ctrl_str);

    let (cols, vals) = ctrl.populate_sql();

    let stmt = &format!(
        "INSERT INTO debs ({}, installed) VALUES ({}, '{}')",
        cols,
        vals,
        installed
    );

    conn.execute(
        stmt
    ).expect("Failed to insert deb");
}

pub fn copy(extract_dir: PathBuf) -> String {
    let mut copied_files: Vec<PathBuf> = vec![];
    let data_dir = extract_dir.join("data");

    for entry in WalkDir::new(&data_dir).into_iter().filter_map(|e| e.ok()) {
        let path = entry.path();
        
        // Skip the data directory itself
        if path == data_dir {
            continue;
        }

        // Get relative path from data/
        let rel = path.strip_prefix(&data_dir).unwrap();
        let dest = Path::new("/").join(rel);

        let result = if entry.file_type().is_dir() {
            std::fs::create_dir_all(&dest)
        } else {
            if let Some(parent) = dest.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            if entry.file_type().is_symlink() {
                let target = std::fs::read_link(path).unwrap();
                if dest.exists() {
                    if dest.is_dir() {
                        warn!("Cannot create symlink {}, a directory with the same name exists.", dest.display());
                        continue;
                    }
                    std::fs::remove_file(&dest).unwrap();
                }
                std::os::unix::fs::symlink(&target, &dest)
            } else { // is_file()
                std::fs::copy(&path, &dest).map(|_| ())
            }
        };

        if let Err(e) = result {
            warn!("Failed to copy {} to {}: {}, skipping...", 
                  path.display(), dest.display(), e);
            continue;
        }

        copied_files.push(dest);
    }

    info!("Copied {} files/directories", copied_files.len());
    copied_files.iter()
        .map(|s| s.display().to_string())
        .collect::<Vec<_>>()
        .join(",")
}

pub fn uninstall(deb: ClioPath, dirs: ProjectDirs, conn: Connection) {
    if !deb.exists() {
        error!("Failed to install .deb file because the .deb file you specified does not exist.");
        std::process::exit(-1);
    }

    if deb.extension().is_none_or(|ext| ext != "deb") {
        error!("Failed to uninstall .deb file because the file you specified isn't one.");
        std::process::exit(-1);
    }

    let f = File::open(deb.to_path_buf()).unwrap();

    let cache_dir = dirs.cache_dir();
    let extract_dir = cache_dir.join("extracted");

    let _ = std::fs::remove_dir_all(&extract_dir);

    let opt_ctrl = extract::extract_control(f);
    if opt_ctrl.is_none() {
        error!("Failed to get control file from .deb, make sure the .deb is valid");
        std::process::exit(-1);
    }

    let ctrl_str = opt_ctrl.unwrap();
    let ctrl = control::parse_control(ctrl_str);
    let installed_ctrl = ControlWithData::from_db(&conn, &ctrl.package, &ctrl.version);

    match installed_ctrl {
        Ok(installed_ctrl) if installed_ctrl.ctrl == ctrl => {
            uninstall_ctrl(installed_ctrl);
            let query = "DELETE FROM debs WHERE package = ? AND version = ?";

            let stmt = conn.prepare(&query);
            let mut stmt = stmt.expect("Failed to prepare delete statement.");

            stmt.bind(1, ctrl.package.as_str()).expect("Failed to bind package name");
            stmt.bind(2, ctrl.version.as_str()).expect("Failed to bind version");
            stmt.next().expect("Failed to execute deletion");
        },

        Err(err) => {
            if let Some(msg) = err.message {
                error!("An error occured while trying to delete the .deb file from the db: {}", msg);
                std::process::exit(-1);
            }
        },

        _ => {}
    }

    info!("Uninstalled .deb package.");
}

pub fn uninstall_ctrl(ctrl: ControlWithData) {
    let installed_paths: Vec<PathBuf> = ctrl.installed
        .split(',')
        .filter(|s| !s.is_empty())
        .map(|s| PathBuf::from(s.trim()))
        .collect();

    for path in installed_paths {
        if let Ok(metadata) = std::fs::symlink_metadata(&path) {
            if metadata.file_type().is_file() || metadata.file_type().is_symlink() {
                if let Err(e) = std::fs::remove_file(&path) {
                    warn!("Failed to remove file/symlink {}: {}", path.display(), e);
                }
            }
        }
    }
}

pub fn is_installed(deb: ClioPath, dirs: ProjectDirs, conn: Connection) {
    if !deb.exists() {
        error!("Failed to install .deb file because the .deb file you specified does not exist.");
        std::process::exit(-1);
    }

    if deb.extension().is_none_or(|ext| ext != "deb") {
        error!("Failed to uninstall .deb file because the file you specified isn't one.");
        std::process::exit(-1);
    }

    let f = File::open(deb.to_path_buf()).unwrap();

    let cache_dir = dirs.cache_dir();
    let extract_dir = cache_dir.join("extracted");

    let _ = std::fs::remove_dir_all(&extract_dir);

    let opt_ctrl = extract::extract_control(f);
    if opt_ctrl.is_none() {
        error!("Failed to get control file from .deb, make sure the .deb is valid");
        std::process::exit(-1);
    }

    let ctrl_str = opt_ctrl.unwrap();
    let ctrl = control::parse_control(ctrl_str);
    let installed_ctrl = ControlWithData::from_db(&conn, &ctrl.package, &ctrl.version);

    match installed_ctrl {
        Ok(installed_ctrl) if installed_ctrl.ctrl == ctrl => {
            info!("The specified package {} installed.", "IS".bold().italic());
        },

        _ => {
            info!("The specified package is {} installed.", "NOT".bold().red().italic());
        }
    }
}
