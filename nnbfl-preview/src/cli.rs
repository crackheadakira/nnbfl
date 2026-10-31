use std::path::{Path, PathBuf};

use clap::{Args, Parser, Subcommand};
use nnbfl::sarc::file::MagicFiles;

use crate::{
    LayoutData,
    archive::{DocStore, EntryPath, LayoutSource, file_stem},
    font::glyph_atlas::GlyphData,
    render::{
        DeviceOptions, FrameRange, OffscreenRenderer, Pose, RenderSettings, export_animation,
    },
};

#[derive(Parser)]
#[command(name = "nnbfl-preview")]
#[command(version)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Lists the layouts in a file with their size and animations, tab separated
    List { file: PathBuf },

    /// Renders layouts to PNG without opening a window
    Render(RenderArgs),
}

#[derive(Args)]
struct RenderArgs {
    /// A .bflyt or an archive holding layouts (SARC, Yaz0 or zstd compressed)
    file: PathBuf,

    /// A folder, or a .png when exactly one image is rendered
    #[arg(short, long)]
    output: PathBuf,

    /// Layouts to render by layout name or file stem, all of them when not given
    #[arg(long)]
    layout: Vec<String>,

    /// Animations to render by name, the layout at rest when neither this nor --all-anims is given
    #[arg(long)]
    anim: Vec<String>,

    #[arg(long, conflicts_with = "anim")]
    all_anims: bool,

    /// A single frame
    #[arg(long, conflicts_with = "frames")]
    frame: Option<f32>,

    /// start:end:step, end defaulting to the animation's last frame and step to 1
    #[arg(long, value_parser = parse_frames)]
    frames: Option<FrameRange>,

    #[arg(long, default_value_t = 1.0)]
    scale: f32,

    /// RRGGBB, RRGGBBAA or `transparent`
    #[arg(long, default_value = "transparent", value_parser = parse_background)]
    background: [f32; 4],

    /// Where parts layouts outside the opened archive are looked up
    #[arg(long)]
    layout_dir: Option<PathBuf>,

    /// Renders on a software adapter
    #[arg(long)]
    software: bool,
}

pub fn run_if_command() -> Option<i32> {
    let first = std::env::args_os().nth(1)?;

    if !matches!(
        first.to_str(),
        Some("list" | "render" | "help" | "--help" | "-h" | "--version" | "-V")
    ) {
        return None;
    }

    Some(match Cli::parse().command {
        Command::List { file } => list(&file),
        Command::Render(args) => render(&args),
    })
}

fn parse_frames(text: &str) -> Result<FrameRange, String> {
    let mut parts = text.split(':');
    let mut number = |name: &str| -> Result<Option<f32>, String> {
        match parts.next().filter(|p| !p.is_empty()) {
            Some(p) => p
                .parse()
                .map(Some)
                .map_err(|_| format!("{name} '{p}' isn't a number")),
            None => Ok(None),
        }
    };

    Ok(FrameRange {
        start: number("start")?.unwrap_or(0.0),
        end: number("end")?,
        step: number("step")?.unwrap_or(1.0),
    })
}

fn parse_background(text: &str) -> Result<[f32; 4], String> {
    if text.eq_ignore_ascii_case("transparent") {
        return Ok([0.0; 4]);
    }

    let hex = text.trim_start_matches('#');
    let channel = |i: usize| {
        u8::from_str_radix(hex.get(i..i + 2).unwrap_or(""), 16)
            .map(|v| f32::from(v) / 255.0)
            .map_err(|_| format!("'{text}' isn't RRGGBB or RRGGBBAA"))
    };

    match hex.len() {
        6 => Ok([channel(0)?, channel(2)?, channel(4)?, 1.0]),
        8 => Ok([channel(0)?, channel(2)?, channel(4)?, channel(6)?]),
        _ => Err(format!("'{text}' isn't RRGGBB or RRGGBBAA")),
    }
}

struct Loaded {
    layout: LayoutData,
    glyphs: GlyphData,
    stem: String,
}

fn load_layouts(file: &Path, layout_dir: Option<&Path>) -> Result<Vec<Loaded>, String> {
    let mut docs = DocStore::default();
    let doc = docs.open(file)?;
    let files = docs.docs[doc].files(&[]);

    let entries: Vec<(EntryPath, String)> = files
        .iter()
        .filter_map(|(path, f)| match f {
            MagicFiles::Bflyt(name, _) => Some((
                path.clone(),
                name.as_deref()
                    .map(file_stem)
                    .unwrap_or_default()
                    .to_string(),
            )),
            _ => None,
        })
        .collect();

    if entries.is_empty() {
        return Err(format!("{} holds no layout", file.display()));
    }

    entries
        .into_iter()
        .map(|(path, stem)| {
            let mut glyphs = GlyphData::default();
            let source = LayoutSource { doc, layout: path };
            let (layout, _) = LayoutData::load_from_buffer(
                files.clone(),
                Some(source),
                layout_dir,
                None,
                &mut glyphs,
            )
            .map_err(|e| format!("{stem}: {e}"))?;

            Ok(Loaded {
                layout,
                glyphs,
                stem,
            })
        })
        .collect()
}

fn anim_name(layout: &LayoutData, index: usize) -> String {
    match &layout.timeline.anim_player.anims[index].name {
        name if name.is_empty() => format!("Animation {}", index + 1),
        name => name.clone(),
    }
}

fn list(file: &Path) -> i32 {
    let layouts = match load_layouts(file, None) {
        Ok(layouts) => layouts,
        Err(e) => {
            eprintln!("{e}");
            return 1;
        }
    };

    for Loaded { layout, .. } in &layouts {
        let size = layout.tree.layout_size;
        println!("layout\t{}\t{}x{}", layout.file_name, size.x, size.y);

        for (i, anim) in layout.timeline.anim_player.anims.iter().enumerate() {
            println!(
                "anim\t{}\t{}\t{}",
                layout.file_name,
                anim_name(layout, i),
                anim.frame_count()
            );
        }
    }

    0
}

fn render(args: &RenderArgs) -> i32 {
    let mut layouts = match load_layouts(&args.file, args.layout_dir.as_deref()) {
        Ok(layouts) => layouts,
        Err(e) => {
            eprintln!("{e}");
            return 1;
        }
    };

    let mut failed = false;

    if !args.layout.is_empty() {
        for wanted in &args.layout {
            if !layouts
                .iter()
                .any(|l| l.layout.file_name == *wanted || l.stem.eq_ignore_ascii_case(wanted))
            {
                eprintln!("No layout named {wanted}");
                failed = true;
            }
        }

        layouts.retain(|l| {
            args.layout
                .iter()
                .any(|w| l.layout.file_name == *w || l.stem.eq_ignore_ascii_case(w))
        });
    }

    let mut renderer = match OffscreenRenderer::new(DeviceOptions {
        software: args.software,
    }) {
        Ok(renderer) => renderer,
        Err(e) => {
            eprintln!("{e}");
            return 1;
        }
    };

    let settings = RenderSettings {
        scale: args.scale,
        background: args.background,
        ..Default::default()
    };
    let range = match (args.frame, args.frames) {
        (Some(frame), _) => FrameRange {
            start: frame,
            end: Some(frame),
            step: 1.0,
        },
        (None, Some(range)) => range,
        (None, None) => FrameRange {
            start: 0.0,
            end: None,
            step: 1.0,
        },
    };

    let animated = args.all_anims || !args.anim.is_empty();
    let single_file = args
        .output
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("png"));

    let one_layout = layouts.len() == 1;

    for Loaded { layout, glyphs, .. } in &mut layouts {
        renderer.prepare(layout, glyphs);
        let name = layout.file_name.clone();

        if !animated {
            let path = if single_file && one_layout {
                args.output.clone()
            } else {
                args.output.join(format!("{name}.png"))
            };

            match renderer
                .render(layout, glyphs, &Pose::default(), &settings)
                .and_then(|image| image.write_png(&path))
            {
                Ok(()) => println!("{}", path.display()),
                Err(e) => {
                    eprintln!("{name}: {e}");
                    failed = true;
                }
            }

            continue;
        }

        let count = layout.timeline.anim_player.anims.len();
        let anims: Vec<usize> = if args.all_anims {
            (0..count).collect()
        } else {
            args.anim
                .iter()
                .filter_map(|wanted| {
                    let found = (0..count).find(|&i| anim_name(layout, i) == *wanted);
                    if found.is_none() {
                        eprintln!("{name}: no animation named {wanted}");
                        failed = true;
                    }
                    found
                })
                .collect()
        };

        if anims.is_empty() {
            continue;
        }

        if single_file && one_layout && anims.len() == 1 && args.frame.is_some() {
            let pose = Pose {
                anim: Some(anims[0]),
                frame: range.start,
            };
            match renderer
                .render(layout, glyphs, &pose, &settings)
                .and_then(|image| image.write_png(&args.output))
            {
                Ok(()) => println!("{}", args.output.display()),
                Err(e) => {
                    eprintln!("{name}: {e}");
                    failed = true;
                }
            }
            continue;
        }

        if single_file {
            eprintln!("{name}: more than one image requested, give a folder as the output");
            failed = true;
            continue;
        }

        for anim in anims {
            let dir = args.output.join(&name).join(anim_name(layout, anim));
            match export_animation(&mut renderer, layout, glyphs, anim, &range, &settings, &dir) {
                Ok(paths) => paths.iter().for_each(|p| println!("{}", p.display())),
                Err(e) => {
                    eprintln!("{name}/{}: {e}", anim_name(layout, anim));
                    failed = true;
                }
            }
        }
    }

    i32::from(failed)
}
