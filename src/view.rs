use std::{fs::File, io::{Cursor, Seek}};

use cli_table::{Cell, CellStruct, Table};
use clio::ClioPath;
use directories::ProjectDirs;
use log::{error, info};

use crate::{control::{self, Control}, extract};

pub fn view(deb: ClioPath, dirs: ProjectDirs) {
    if !deb.exists() {
        error!("Failed to view .deb file because the .deb file you specified does not exist.");
        std::process::exit(-1);
    }

    if deb.extension().is_none_or(|ext| ext != "deb") {
        error!("Failed to view .deb file because the file you specified isn't one.");
        std::process::exit(-1);
    }

    let mut f = File::open(deb.to_path_buf()).unwrap();

    let cache_dir = dirs.cache_dir();
    let extract_dir = cache_dir.join("extracted");

    let _ = std::fs::remove_dir_all(&extract_dir);

    let ctrl_str = extract::extract_control(f.try_clone().expect("Failed to clone file")).expect("Failed to extract control");
    let ctrl = control::parse_control(ctrl_str);

    f.seek(std::io::SeekFrom::Start(0)).unwrap();

    let tree = extract::extract_files_tree(f);
    let mut table: Vec<Vec<CellStruct>> = vec![];

    for field in Control::fields() {
        let val = ctrl.field(field.as_str()).unwrap();
        let val = if val == "NULL".to_string() {
            continue;
        } else {
            truncate(&val, 50)
        };

        table.push(vec![field.clone().cell(), val.cell()]);
    }

    info!("control:");
    cli_table::print_stdout(table.table()).expect("Failed to print table of control fields");
    info!("files:");

    let mut buf = Cursor::new(Vec::new());

    ptree::write_tree(&tree, &mut buf).expect("Failed to write file tree");

    let out = String::from_utf8(buf.into_inner()).expect("invalid UTF-8");

    println!("{}", out);
}

pub fn truncate(s: &str, max_len: usize) -> String {
    if s.len() > max_len {
        format!("{}...", &s[..max_len])
    } else {
        s.to_string()
    }
}
