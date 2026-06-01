use std::env;
use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::exit;

mod bflan;
mod bflan_writer;

use bflan::BflanFile;
use bflan_writer::serialize_bflan;

pub const fn tchar_code32(b: &[u8; 4]) -> u32 {
    (b[0] as u32) | ((b[1] as u32) << 8) | ((b[2] as u32) << 16) | ((b[3] as u32) << 24)
}

fn print_usage() {
    println!("Usage:");
    println!("\t./bflan extract <input_file_or_dir> <output_file_or_dir>",);
    println!("\t./bflan pack <input_file_or_dir> <output_file_or_dir>",);
    println!("\t./bflan test <input_file_or_dir>",);
}

fn find_files(dir: &Path, target_ext: &str, files: &mut Vec<PathBuf>) {
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                find_files(&path, target_ext, files);
            } else if path.is_file() {
                if let Some(ext) = path.extension() {
                    if ext == target_ext {
                        files.push(path);
                    }
                }
            }
        }
    }
}

fn extract_file(input_path: &Path, output_path: &Path) {
    let file_in = match fs::read(input_path) {
        Ok(bytes) => bytes,
        Err(e) => {
            eprintln!("Failed to read {:?}: {}", input_path, e);
            return;
        }
    };

    let bflan_file = match BflanFile::parse_file(&file_in) {
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

fn pack_file(input_path: &Path, output_path: &Path) {
    let json_string = match fs::read_to_string(input_path) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Failed to read {:?}: {}", input_path, e);
            return;
        }
    };

    let json_data: BflanFile = match serde_json::from_str(&json_string) {
        Ok(data) => data,
        Err(e) => {
            eprintln!("Failed to deserialize {:?}: {}", input_path, e);
            return;
        }
    };

    let writer = serialize_bflan(json_data);
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
            extract_file(&path, &out_path);
        } else {
            pack_file(&path, &out_path);
        }
    }

    println!("Batch {} complete!", command);
}

fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() < 3 {
        print_usage();
        exit(1);
    }

    let command = args[1].as_str();
    let input_path = Path::new(&args[2]);

    if !input_path.exists() {
        eprintln!("Error: Input path does not exist.");
        exit(1);
    }

    match command {
        "extract" | "pack" => {
            if args.len() < 4 {
                eprintln!("No output dir specified");
                print_usage();
                exit(1);
            }

            let output_path = Path::new(&args[3]);
            if input_path.is_dir() {
                process_batch(command, input_path, output_path);
            } else {
                if let Some(parent) = output_path.parent() {
                    let _ = fs::create_dir_all(parent);
                }
                if command == "extract" {
                    extract_file(input_path, output_path);
                } else {
                    pack_file(input_path, output_path);
                }
            }
        }
        "test" => {
            let mut blan_files = Vec::new();
            if input_path.is_dir() {
                find_files(input_path, "bflan", &mut blan_files);
            } else {
                blan_files.push(input_path.into());
            }

            test_roundtrip(input_path, blan_files);
        }
        _ => {
            eprintln!("Unknown command: {}", command);
            print_usage();
            exit(1);
        }
    }
}

fn test_roundtrip(input_dir: &Path, bflan_files: Vec<PathBuf>) {
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

            let file = match BflanFile::parse_file(&file_in) {
                Ok(res) => res,
                Err(e) => {
                    eprintln!("Failed to parse: {e}");
                    fail_count += 1;
                    continue;
                }
            };

            let writer = serialize_bflan(file);
            let file_out = writer.buffer;

            if file_in == file_out {
                // println!("\tRound trip successful");
                success_count += 1;
            } else {
                println!("{file_name:?}");
                println!("Original length:\t{} bytes", file_in.len());
                println!("New length:\t\t{} bytes", file_out.len());

                for i in 0..std::cmp::min(file_in.len(), file_out.len()) {
                    if i >= 0x0C && i <= 0x0F {
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

                        println!("Struct Context: {last_mark}\n");

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
