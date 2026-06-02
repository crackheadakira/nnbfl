use clap::{Parser, Subcommand};
use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::exit;

use crate::bflan::file::Bflan;

mod bflan;
mod bflyt;
mod core;
mod ui2d;

#[derive(Parser)]
#[command(name = "nnbfl")]
#[command(version)]
struct Cli {
    #[command(subcommand)]
    format: Format,
}

#[derive(Subcommand)]
enum Format {
    /// Work with BFLAN (Animation) files
    Bflan {
        #[command(subcommand)]
        action: Action,
    },

    /// Work with BFLYT (Layout) files
    Bflyt {
        #[command(subcommand)]
        action: Action,
    },
}

#[derive(Subcommand)]
enum Action {
    /// Extracts a binary file to JSON
    Extract { input: PathBuf, output: PathBuf },

    /// Packs a JSON file into binary
    Pack { input: PathBuf, output: PathBuf },

    /// Runs a binary-accurate roundtrip test
    Test { input: PathBuf },
}

fn main() {
    let cli = Cli::parse();

    // Route the format first
    match &cli.format {
        Format::Bflan { action } => {
            handle_action("bflan", action);
        }
        Format::Bflyt { action } => {
            handle_action("bflyt", action);
        }
    }
}

fn handle_action(ext: &str, action: &Action) {
    match action {
        Action::Extract { input, output } => {
            validate_input(input);
            process_command("extract", ext, input, output);
        }
        Action::Pack { input, output } => {
            validate_input(input);
            process_command("pack", ext, input, output);
        }
        Action::Test { input } => {
            validate_input(input);
            let mut files = Vec::new();
            if input.is_dir() {
                find_files(input, ext, &mut files);
            } else {
                files.push(input.clone());
            }

            match ext {
                "bflan" => test_roundtrip_bflan(input, files),
                "bflyt" => {
                    // test_roundtrip_bflyt(input, files)
                    println!("BFLYT testing not yet implemented!");
                }
                _ => unreachable!(),
            }
        }
    }
}

fn validate_input(input: &Path) {
    if !input.exists() {
        eprintln!("Error: Input path {:?} does not exist.", input);
        exit(1);
    }
}

fn process_command(command: &str, ext: &str, input_path: &Path, output_path: &Path) {
    if input_path.is_dir() {
        process_batch(command, input_path, output_path);
    } else {
        if let Some(parent) = output_path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }

        match (command, ext) {
            ("extract", "bflan") => extract_bflan_file(input_path, output_path),
            ("pack", "bflan") => pack_bflan_file(input_path, output_path),

            ("extract", "bflyt") => println!("BFLYT extract not yet implemented!"),
            ("pack", "bflyt") => println!("BFLYT pack not yet implemented!"),

            _ => unreachable!(),
        }
    }
}

fn test_roundtrip_bflan(input_dir: &Path, bflan_files: Vec<PathBuf>) {
    let mut success_count = 0;
    let mut fail_count = 0;

    for path in bflan_files {
        let entry = path.strip_prefix(input_dir).unwrap_or(&path);

        if path.is_file() {
            if let Some(ext) = path.extension() {
                if ext != "bflan" {
                    continue;
                }
            } else {
                continue;
            }

            let file_name = entry.file_name().unwrap_or(OsStr::new("Unknown Name"));

            let file_in = match fs::read(&path) {
                Ok(bytes) => bytes,
                Err(e) => {
                    eprintln!("Failed to read: {e}");
                    fail_count += 1;
                    continue;
                }
            };

            let file = match Bflan::parse(&file_in) {
                Ok(res) => res,
                Err(e) => {
                    eprintln!("Failed to parse: {e}");
                    fail_count += 1;
                    continue;
                }
            };

            let writer = file.serialize();
            let file_out = writer.buffer;

            if file_in == file_out {
                success_count += 1;
            } else {
                println!("{file_name:?}");
                if file_in.len() != file_out.len() {
                    println!("Original length:\t{} bytes", file_in.len());
                    println!("New length:\t\t{} bytes", file_out.len());
                }

                for i in 0..std::cmp::min(file_in.len(), file_out.len()) {
                    // skip header file size
                    if (0x0C..=0x0F).contains(&i) {
                        continue;
                    }

                    if file_in[i] != file_out[i] {
                        println!(
                            "First difference at offset 0x{i:X}: expected 0x{:02X}, got 0x{:02X}",
                            file_in[i], file_out[i]
                        );

                        let mut last_mark = "Unknown Location";

                        for (pos, name) in &writer.breadcrumbs {
                            if *pos <= i {
                                last_mark = name;
                            } else {
                                break;
                            }
                        }

                        println!("Context: {last_mark}\n");

                        break;
                    }
                }
                fail_count += 1;
            }
        }
    }

    println!("Total successful:\t{success_count}");
    println!("Total failed:\t\t{fail_count}");
}

fn find_files(dir: &Path, target_ext: &str, files: &mut Vec<PathBuf>) {
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                find_files(&path, target_ext, files);
            } else if path.is_file()
                && let Some(ext) = path.extension()
                && ext == target_ext
            {
                files.push(path);
            }
        }
    }
}

fn extract_bflan_file(input_path: &Path, output_path: &Path) {
    let file_in = match fs::read(input_path) {
        Ok(bytes) => bytes,
        Err(e) => {
            eprintln!("Failed to read {:?}: {}", input_path, e);
            return;
        }
    };

    let bflan_file = match Bflan::parse(&file_in) {
        Ok(res) => res,
        Err(e) => {
            eprintln!("Failed to parse {:?}: {}", input_path, e);
            return;
        }
    };

    let json_string = match serde_json::to_string_pretty(&bflan_file) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Failed to serialize {:?}: {}", input_path, e);
            return;
        }
    };

    if let Err(e) = fs::write(output_path, json_string) {
        eprintln!("Failed to write output {:?}: {}", output_path, e);
    } else {
        println!("Extracted: {:?}", input_path.file_name().unwrap());
    }
}

fn pack_bflan_file(input_path: &Path, output_path: &Path) {
    let json_string = match fs::read_to_string(input_path) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Failed to read {:?}: {}", input_path, e);
            return;
        }
    };

    let json_data: Bflan = match serde_json::from_str(&json_string) {
        Ok(data) => data,
        Err(e) => {
            eprintln!("Failed to deserialize {:?}: {}", input_path, e);
            return;
        }
    };

    let writer = json_data.serialize();
    let file_out = writer.buffer;

    if let Err(e) = fs::write(output_path, file_out) {
        eprintln!("Failed to write output {:?}: {}", output_path, e);
    } else {
        println!("Packed: {:?}", input_path.file_name().unwrap());
    }
}

fn process_batch(command: &str, in_dir: &Path, out_dir: &Path) {
    if let Err(e) = fs::create_dir_all(out_dir) {
        eprintln!("Failed to create output directory {:?}: {}", out_dir, e);
        exit(1);
    }

    let mut target_files = Vec::new();
    let ext = if command == "extract" {
        "bflan"
    } else {
        "json"
    };

    find_files(in_dir, ext, &mut target_files);

    if target_files.is_empty() {
        println!("No .{} files found in {:?}", ext, in_dir);
        return;
    }

    println!("Found {} files. Processing...", target_files.len());

    for path in target_files {
        let relative_path = path.strip_prefix(in_dir).unwrap_or(&path);
        let mut out_path = out_dir.join(relative_path);

        if command == "extract" {
            out_path.set_extension("json");
        } else {
            out_path.set_extension("bflan");
        }

        if let Some(parent) = out_path.parent() {
            let _ = fs::create_dir_all(parent);
        }

        if command == "extract" {
            extract_bflan_file(&path, &out_path);
        } else {
            pack_bflan_file(&path, &out_path);
        }
    }

    println!("Batch {} complete!", command);
}
