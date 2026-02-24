use std::{collections::HashMap, fs::{self, File}, io::{Read, Seek}, path::{Path, PathBuf}};

use ar::Archive;
use indicatif::{ProgressBar, ProgressStyle};
use ptree::TreeBuilder;
use tar::{Archive as TarArchive, EntryType};

use bzip2::read::BzDecoder;
use flate2::read::GzDecoder;
use xz2::read::XzDecoder;
use zstd::stream::read::Decoder as ZstdDecoder;

pub fn extract_to(extract_dir: PathBuf, f: File) {
    let _ = fs::create_dir_all(&extract_dir); // error silently

    let mut f = f.try_clone().expect("Failed to clone file");

    let files = count(&f);
    let bar = ProgressBar::new(files as u64);

    bar.set_style(ProgressStyle::default_bar()
        .template("{spinner:.green} [{elapsed_precise}] [{percent_precise}] [{wide_bar:.cyan/blue}] {pos}/{human_len} ({eta}) {msg}")
        .unwrap()
        .progress_chars("#>-"));

    let _ = f.seek(std::io::SeekFrom::Start(0));

    let mut archive = Archive::new(f.try_clone().expect("Failed to clone file"));

    while let Some(entry) = archive.next_entry().transpose().expect("Failed to transpose new entry") {
        let name = String::from_utf8_lossy(entry.header().identifier())
            .trim()
            .trim_end_matches('/')
            .to_string();

        let decoder: Option<Box<dyn Read>> = if name.ends_with(".tar.gz") {
            Some(Box::new(GzDecoder::new(entry)))
        } else if name.ends_with(".tar.xz") {
            Some(Box::new(XzDecoder::new(entry)))
        } else if name.ends_with(".tar.bz2") {
            Some(Box::new(BzDecoder::new(entry)))
        } else if name.ends_with(".tar.zst") {
            ZstdDecoder::new(entry)
                .ok()
                .map(|decoder| Box::new(decoder) as Box<dyn Read>)
        } else {
            None
        };

        if let Some(decoder) = decoder {
            let mut tar = TarArchive::new(decoder);
            let dst = extract_dir.join(
                Path::new(&name)
                    .file_name()
                    .and_then(|s| s.to_str())
                    .and_then(|s| s.split('.').next())
                    .unwrap_or("")
            );

            if dst.symlink_metadata().is_err() {
                fs::create_dir_all(&dst)
                    .expect("Failed to create_dir_all");
            }

            let dst = &dst.canonicalize().unwrap_or(dst.to_path_buf());

            let mut directories = Vec::new();
            for entry in tar.entries().expect("Failed to get tar entries") {
                let mut file = entry.expect("Failed to iterate over archive");
                if file.header().entry_type() == EntryType::Directory {
                    directories.push(file);
                } else {
                    file.unpack_in(dst).expect("Failed to unpack in dst");
                    bar.inc(1);
                }
            }

            directories.sort_by(|a, b| b.path_bytes().cmp(&a.path_bytes()));
            for mut dir in directories {
                dir.unpack_in(dst).expect("Failed to unpack inner file");
                bar.inc(1);
            }

            // tar.unpack(dst).expect("Failed to unpack tar");
        }
    }

    bar.finish();
}

pub fn count(f: &File) -> usize {
    let mut total = 0;
    let mut archive = Archive::new(f);

    while let Some(entry) = archive.next_entry().transpose().expect("Failed to transpose new entry") {
        let name = String::from_utf8_lossy(entry.header().identifier())
            .trim()
            .trim_end_matches('/')
            .to_string();

        let decoder: Option<Box<dyn Read>> = if name.ends_with(".tar.gz") {
            Some(Box::new(GzDecoder::new(entry)))
        } else if name.ends_with(".tar.xz") {
            Some(Box::new(XzDecoder::new(entry)))
        } else if name.ends_with(".tar.bz2") {
            Some(Box::new(BzDecoder::new(entry)))
        } else if name.ends_with(".tar.zst") {
            ZstdDecoder::new(entry)
                .ok()
                .map(|decoder| Box::new(decoder) as Box<dyn Read>)
        } else {
            None
        };

        if let Some(decoder) = decoder {
            let mut tar = TarArchive::new(decoder);

            total += tar.entries().unwrap().count();
        }
    }

    total
}

pub fn extract_control(f: File) -> Option<String> {
    let mut archive = Archive::new(f);

    while let Some(entry) = archive.next_entry().transpose().ok()? {
        let name = String::from_utf8_lossy(entry.header().identifier())
            .trim()
            .trim_end_matches('/')
            .to_string();

        let decoder: Option<Box<dyn Read>> = if name == "control.tar.gz" {
            Some(Box::new(GzDecoder::new(entry)))
        } else if name == "control.tar.xz" {
            Some(Box::new(XzDecoder::new(entry)))
        } else if name == "control.tar.bz2" {
            Some(Box::new(BzDecoder::new(entry)))
        } else if name == "control.tar.zst" {
            ZstdDecoder::new(entry)
                .ok()
                .map(|decoder| Box::new(decoder) as Box<dyn Read>)
        } else {
            None
        };

        if let Some(decoder) = decoder {
            let mut tar = TarArchive::new(decoder);

            for entry in tar.entries().ok()? {
                let mut file = entry.ok()?;
                let path = file.path().ok()?;

                if let Some(fname) = path.file_name() && fname == "control" {
                    let mut contents = String::new();
                    file.read_to_string(&mut contents).ok()?;
                    return Some(contents);
                }
            }
        }
    }

    None
}

pub fn extract_files_tree(f: File) -> ptree::item::StringItem {
    let mut archive = Archive::new(f);

    let mut builder = TreeBuilder::new("package".to_string());

    while let Some(entry) = archive.next_entry().transpose().expect("ar read fail") {
        let name = String::from_utf8_lossy(entry.header().identifier())
            .trim()
            .trim_end_matches('/')
            .to_string();
        
        let decoder: Option<Box<dyn Read>> = if name.ends_with(".tar.gz") {
            Some(Box::new(GzDecoder::new(entry)))
        } else if name.ends_with(".tar.xz") {
            Some(Box::new(XzDecoder::new(entry)))
        } else if name.ends_with(".tar.bz2") {
            Some(Box::new(BzDecoder::new(entry)))
        } else if name.ends_with(".tar.zst") {
            ZstdDecoder::new(entry)
                .ok()
                .map(|decoder| Box::new(decoder) as Box<dyn Read>)
        } else {
            None
        };

        if let Some(dec) = decoder {
            let mut subtree = builder.begin_child(name.clone());
            let mut tar = TarArchive::new(dec);

            // Collect all paths first
            let mut paths = Vec::new();
            for entry in tar.entries().expect("tar entries fail") {
                if let Ok(file) = entry {
                    if let Ok(path) = file.path() {
                        paths.push(path.display().to_string());
                    }
                }
            }
            
            // Build tree from paths
            build_tree_from_paths(&mut subtree, paths);
            
            builder.end_child();
        } else {
            builder.add_empty_child(name);
        }
    }

    builder.build()
}

fn build_tree_from_paths(builder: &mut TreeBuilder, paths: Vec<String>) {
    // Build a directory structure
    let mut root: HashMap<String, Node> = HashMap::new();
    
    for path in paths {
        let parts: Vec<&str> = path.split('/').filter(|s| !s.is_empty() && *s != ".").collect();
        insert_path(&mut root, &parts);
    }
    
    // Convert to tree
    add_nodes_to_tree(builder, &root);
}

#[derive(Default)]
struct Node {
    children: HashMap<String, Node>,
    is_file: bool,
}

fn insert_path(node: &mut HashMap<String, Node>, parts: &[&str]) {
    if parts.is_empty() {
        return;
    }
    
    let first = parts[0].to_string();
    let entry = node.entry(first.clone()).or_insert_with(Node::default);
    
    if parts.len() == 1 {
        entry.is_file = true;
    } else {
        insert_path(&mut entry.children, &parts[1..]);
    }
}

fn add_nodes_to_tree(builder: &mut TreeBuilder, nodes: &HashMap<String, Node>) {
    let mut sorted_keys: Vec<_> = nodes.keys().collect();
    sorted_keys.sort();
    
    for key in sorted_keys {
        let node = &nodes[key];
        
        if node.children.is_empty() {
            builder.add_empty_child(key.clone());
        } else {
            let mut child = builder.begin_child(key.clone());
            add_nodes_to_tree(&mut child, &node.children);
            builder.end_child();
        }
    }
}
