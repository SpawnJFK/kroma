//! `kroma` - headless Kroma: run actions over folders of images, convert formats, inspect files.

use anyhow::{bail, Context, Result};
use clap::{Parser, Subcommand};
use kroma_core::filter::{Filter, FilterCtx};
use kroma_core::ops::{Action, Op};
use rayon::prelude::*;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Instant;

#[derive(Parser)]
#[command(name = "kroma", version, about = "Kroma image editor - command line")]
struct Cli {
    #[command(subcommand)]
    cmd: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Apply an action (JSON recipe and/or inline ops) to many images in parallel.
    Run {
        /// Action JSON file ({"name":..,"ops":[..]} or a bare list of ops).
        #[arg(short, long)]
        action: Option<PathBuf>,
        /// Built-in action by name (see `kroma actions`).
        #[arg(short, long)]
        builtin: Option<String>,
        /// Inline op as JSON, repeatable, e.g. --op '{"op":"auto_tone"}'.
        #[arg(long = "op")]
        ops: Vec<String>,
        /// Output directory (created if missing).
        #[arg(short, long)]
        out: PathBuf,
        /// Output format extension (png, jpg, webp, tiff, ...). Default: keep input format, or png for .kroma/.psd/.ora.
        #[arg(short, long)]
        format: Option<String>,
        /// JPEG/WebP quality 1-100.
        #[arg(short, long, default_value_t = 90)]
        quality: u8,
        /// Suffix added to output file names.
        #[arg(long, default_value = "")]
        suffix: String,
        /// Worker threads (default: all cores).
        #[arg(short = 'j', long)]
        jobs: Option<usize>,
        /// Input files (folders are expanded).
        inputs: Vec<PathBuf>,
    },
    /// Convert / flatten one file to another format (by extension). .kroma and .ora stay layered.
    Convert {
        input: PathBuf,
        output: PathBuf,
        #[arg(short, long, default_value_t = 90)]
        quality: u8,
    },
    /// Print size, layers and blend modes of a file.
    Info { input: PathBuf },
    /// List built-in actions; with --dump NAME print its JSON (a template for your own).
    Actions {
        #[arg(long)]
        dump: Option<String>,
    },
    /// Print a JSON template for every op, adjustment and filter (for writing recipes).
    Ops,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.cmd {
        Command::Run {
            action,
            builtin,
            ops,
            out,
            format,
            quality,
            suffix,
            jobs,
            inputs,
        } => {
            let mut act = match (action, builtin) {
                (Some(p), _) => {
                    Action::load(&p).with_context(|| format!("reading {}", p.display()))?
                }
                (None, Some(n)) => Action::builtins()
                    .into_iter()
                    .find(|a| a.name.to_lowercase().contains(&n.to_lowercase()))
                    .with_context(|| {
                        format!("no built-in action matching '{n}' (see `kroma actions`)")
                    })?,
                (None, None) => Action {
                    name: "inline".into(),
                    ops: vec![],
                },
            };
            for o in ops {
                let op: Op =
                    serde_json::from_str(&o).with_context(|| format!("parsing --op {o}"))?;
                act.ops.push(op);
            }
            if act.ops.is_empty() {
                bail!("nothing to do: pass --action, --builtin or --op");
            }
            let files = expand(&inputs)?;
            if files.is_empty() {
                bail!("no input images found");
            }
            std::fs::create_dir_all(&out)?;
            if let Some(j) = jobs {
                rayon::ThreadPoolBuilder::new()
                    .num_threads(j)
                    .build_global()
                    .ok();
            }
            let total = files.len();
            let done = AtomicUsize::new(0);
            let failed = AtomicUsize::new(0);
            let start = Instant::now();
            eprintln!(
                "kroma: running '{}' ({} ops) on {} files → {}",
                act.name,
                act.ops.len(),
                total,
                out.display()
            );
            files.par_iter().for_each(|f| {
                let t0 = Instant::now();
                let r = process(f, &act, &out, format.as_deref(), quality, &suffix);
                let n = done.fetch_add(1, Ordering::SeqCst) + 1;
                let el = start.elapsed().as_secs_f32();
                let eta = el / n as f32 * (total - n) as f32;
                match r {
                    Ok(p) => println!(
                        "[{n}/{total} {:>3.0}% eta {:.0}s] {} → {} ({} ms)",
                        n as f32 / total as f32 * 100.0,
                        eta,
                        f.display(),
                        p.display(),
                        t0.elapsed().as_millis()
                    ),
                    Err(e) => {
                        failed.fetch_add(1, Ordering::SeqCst);
                        println!("[{n}/{total}] FAILED {}: {e:#}", f.display());
                    }
                }
            });
            let fl = failed.load(Ordering::SeqCst);
            eprintln!(
                "kroma: done in {:.1}s, {} ok, {} failed",
                start.elapsed().as_secs_f32(),
                total - fl,
                fl
            );
            if fl > 0 {
                std::process::exit(1);
            }
        }
        Command::Convert {
            input,
            output,
            quality,
        } => {
            let doc = kroma_core::io::open(&input)?;
            let ext = output
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or("")
                .to_ascii_lowercase();
            match ext.as_str() {
                "kroma" | "ora" => kroma_core::io::save(&doc, &output)?,
                _ => kroma_core::io::export(
                    &doc,
                    &output,
                    &kroma_core::io::ExportOptions {
                        quality,
                        ..Default::default()
                    },
                )?,
            }
            println!("{} → {}", input.display(), output.display());
        }
        Command::Info { input } => {
            let doc = kroma_core::io::open(&input)?;
            println!(
                "{}: {} × {} px, {} dpi, {} layers",
                input.display(),
                doc.width,
                doc.height,
                doc.dpi,
                doc.layer_count()
            );
            for (depth, l) in doc.flat_list() {
                println!(
                    "{}{} [{}] {} {:.0}%{}{}{}",
                    "  ".repeat(depth + 1),
                    l.name,
                    l.kind_label(),
                    l.blend.name(),
                    l.opacity * 100.0,
                    if l.visible { "" } else { " hidden" },
                    if l.mask().is_some() { " +mask" } else { "" },
                    if l.effects.is_empty() {
                        String::new()
                    } else {
                        format!(
                            " fx:{}",
                            l.effects
                                .iter()
                                .map(|e| e.name())
                                .collect::<Vec<_>>()
                                .join(",")
                        )
                    }
                );
            }
        }
        Command::Actions { dump } => match dump {
            Some(n) => {
                let a = Action::builtins()
                    .into_iter()
                    .find(|a| a.name.to_lowercase().contains(&n.to_lowercase()))
                    .context("no such action")?;
                println!("{}", serde_json::to_string_pretty(&a)?);
            }
            None => {
                for a in Action::builtins() {
                    println!(
                        "{:<40} {}",
                        a.name,
                        a.ops
                            .iter()
                            .map(|o| o.label())
                            .collect::<Vec<_>>()
                            .join(" → ")
                    );
                }
            }
        },
        Command::Ops => {
            let mut v: Vec<Op> = vec![
                Op::AutoTone,
                Op::AutoContrast,
                Op::AutoColor,
                Op::Resize {
                    width: Some(1600),
                    height: None,
                    percent: None,
                    method: Default::default(),
                },
                Op::FitWithin {
                    max_width: 2048,
                    max_height: 2048,
                },
                Op::CanvasSize {
                    width: 2000,
                    height: 2000,
                    anchor: Default::default(),
                },
                Op::Crop {
                    x: 0,
                    y: 0,
                    width: 800,
                    height: 600,
                },
                Op::CropAspect {
                    ratio_w: 16.0,
                    ratio_h: 9.0,
                },
                Op::Trim,
                Op::RotateCw,
                Op::Rotate { degrees: 3.5 },
                Op::FlipHorizontal,
                Op::SelectSubject,
                Op::ContentAwareFill,
                Op::Watermark {
                    text: "© You".into(),
                    size: 32.0,
                    color: kroma_core::Color::WHITE,
                    opacity: 0.6,
                    anchor: kroma_core::transform::Anchor::BottomRight,
                    margin: 24.0,
                },
                Op::Flatten,
            ];
            for a in kroma_core::adjust::Adjustment::catalog() {
                v.push(Op::Adjust { adjustment: a });
            }
            for (_, fs) in Filter::menu() {
                for f in fs {
                    v.push(Op::Filter { filter: f });
                }
            }
            use std::io::Write;
            let mut so = std::io::stdout().lock();
            for op in v {
                if writeln!(so, "{}", serde_json::to_string(&op)?).is_err() {
                    break; // e.g. piped into `head`
                }
            }
        }
    }
    Ok(())
}

fn expand(inputs: &[PathBuf]) -> Result<Vec<PathBuf>> {
    let mut out = Vec::new();
    for p in inputs {
        if p.is_dir() {
            for e in std::fs::read_dir(p)? {
                let e = e?.path();
                let ok = e
                    .extension()
                    .and_then(|x| x.to_str())
                    .map(|x| {
                        kroma_core::io::OPEN_EXTENSIONS.contains(&x.to_ascii_lowercase().as_str())
                    })
                    .unwrap_or(false);
                if ok {
                    out.push(e);
                }
            }
        } else {
            out.push(p.clone());
        }
    }
    out.sort();
    Ok(out)
}

fn process(
    f: &Path,
    act: &Action,
    out: &Path,
    format: Option<&str>,
    quality: u8,
    suffix: &str,
) -> Result<PathBuf> {
    let mut doc = kroma_core::io::open(f)?;
    act.run(&mut doc, &FilterCtx::default())?;
    let in_ext = f
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("png")
        .to_ascii_lowercase();
    let ext = match format {
        Some(e) => e.trim_start_matches('.').to_ascii_lowercase(),
        None if kroma_core::io::EXPORT_EXTENSIONS.contains(&in_ext.as_str()) => in_ext,
        None => "png".into(),
    };
    let stem = f
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "out".into());
    let mut target = out.join(format!("{stem}{suffix}.{ext}"));
    if target.canonicalize().ok() == f.canonicalize().ok() {
        target = out.join(format!("{stem}{suffix}_kroma.{ext}"));
    }
    kroma_core::io::export(
        &doc,
        &target,
        &kroma_core::io::ExportOptions {
            quality,
            ..Default::default()
        },
    )?;
    Ok(target)
}
