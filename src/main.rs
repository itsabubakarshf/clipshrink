mod cli;
mod encode;
mod ffmpeg;
mod process;
mod util;

use std::io::{self, IsTerminal, Write};
use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::{bail, Result};
use clap::Parser;
use indicatif::{ProgressBar, ProgressStyle};
use walkdir::WalkDir;

use cli::{Cli, Mode};
use encode::EncodeSettings;
use ffmpeg::Tools;
use process::{process_file, Options, Outcome};
use util::fmt_size;

const VIDEO_EXTS: &[&str] = &["mp4", "mkv", "mov", "avi", "webm", "m4v", "wmv", "flv", "ts", "mts", "m2ts", "mpg", "mpeg", "3gp"];

fn main() {
    let interactive = std::env::args().len() == 1;
    let code = match run() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("error: {e:#}");
            2
        }
    };
    if interactive && io::stdin().is_terminal() {
        // Launched by double-click: keep the window open.
        print!("\nPress Enter to exit...");
        let _ = io::stdout().flush();
        let _ = io::stdin().read_line(&mut String::new());
    }
    std::process::exit(code);
}

fn run() -> Result<i32> {
    let mut cli = Cli::parse();
    if cli.input.is_none() {
        wizard(&mut cli)?;
    }
    let input = cli.input.clone().unwrap();
    if !input.exists() {
        bail!("'{}' does not exist", input.display());
    }

    let tools = Tools::locate(cli.ffmpeg.as_deref())?;

    let needs_encode = cli.mode != Mode::Split;
    let encoder = if needs_encode { tools.resolve_encoder(cli.encoder, cli.codec)? } else { cli::EncoderKind::Cpu };
    let opts = Options {
        mode: cli.mode,
        limit: cli.max_size,
        overwrite: cli.overwrite,
        encode: EncodeSettings {
            codec: cli.codec,
            encoder,
            crf: cli.crf.unwrap_or_else(|| EncodeSettings::default_crf(cli.codec)),
            preset: cli.preset.clone(),
            height: cli.height,
            audio_bitrate: cli.audio_bitrate.clone(),
        },
    };

    // Work out the list of (source file, output folder).
    let base = if input.is_dir() { input.clone() } else { input.parent().map(Path::to_path_buf).unwrap_or_default() };
    let out_root = cli.output.clone().unwrap_or_else(|| base.join("compressor_out"));
    let files = collect_videos(&input, cli.recursive, &out_root);
    if files.is_empty() {
        bail!("no video files found in '{}'{}", input.display(), if input.is_dir() && !cli.recursive { " (use --recursive for sub-folders)" } else { "" });
    }

    println!(
        "Mode: {:?} | max part size: {} | output: {}{}",
        cli.mode,
        fmt_size(opts.limit),
        out_root.display(),
        if needs_encode { format!(" | encoder: {:?}/{:?}", opts.encode.codec, opts.encode.encoder) } else { String::new() }
    );

    let mut failed = 0;
    let mut total_in = 0u64;
    let mut total_out = 0u64;
    for (i, file) in files.iter().enumerate() {
        let rel_dir = file.strip_prefix(&base).ok().and_then(|r| r.parent()).unwrap_or(Path::new(""));
        let out_dir = out_root.join(rel_dir);
        let name = file.file_name().unwrap().to_string_lossy().to_string();
        println!("\n[{}/{}] {name}", i + 1, files.len());

        let pb = ProgressBar::new(1);
        pb.set_style(
            ProgressStyle::with_template("  {msg:<30} [{bar:30.cyan/blue}] {percent:>3}%  {elapsed_precise}")
                .unwrap()
                .progress_chars("=> "),
        );
        pb.enable_steady_tick(Duration::from_millis(200));

        match process_file(&tools, &opts, file, &out_dir, &pb) {
            Ok(Outcome::Done { in_size, out_size, parts }) => {
                pb.finish_and_clear();
                total_in += in_size;
                total_out += out_size;
                println!(
                    "  done: {} -> {}{}",
                    fmt_size(in_size),
                    fmt_size(out_size),
                    if parts > 1 { format!(" in {parts} parts") } else { String::new() }
                );
            }
            Ok(Outcome::Skipped(why)) => {
                pb.finish_and_clear();
                println!("  skipped: {why}");
            }
            Err(e) => {
                pb.abandon();
                failed += 1;
                eprintln!("  FAILED: {e:#}");
            }
        }
    }

    println!("\nFinished {} file(s), {} failed. Total {} -> {}. Output: {}", files.len(), failed, fmt_size(total_in), fmt_size(total_out), out_root.display());
    Ok(if failed > 0 { 1 } else { 0 })
}

fn collect_videos(input: &Path, recursive: bool, out_root: &Path) -> Vec<PathBuf> {
    let is_video = |p: &Path| {
        p.extension().and_then(|e| e.to_str()).map(|e| VIDEO_EXTS.contains(&e.to_ascii_lowercase().as_str())).unwrap_or(false)
    };
    if input.is_file() {
        return vec![input.to_path_buf()];
    }
    let out_canon = std::fs::canonicalize(out_root).ok();
    let mut v: Vec<PathBuf> = WalkDir::new(input)
        .max_depth(if recursive { usize::MAX } else { 1 })
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file() && is_video(e.path()))
        .map(|e| e.into_path())
        .filter(|p| match (&out_canon, std::fs::canonicalize(p)) {
            (Some(o), Ok(c)) => !c.starts_with(o),
            _ => true,
        })
        // skip our own leftovers
        .filter(|p| !p.to_string_lossy().ends_with(".part.mp4"))
        .collect();
    v.sort();
    v
}

// ---------- interactive mode ----------

fn prompt(msg: &str, default: &str) -> String {
    if default.is_empty() {
        print!("{msg}: ");
    } else {
        print!("{msg} [{default}]: ");
    }
    let _ = io::stdout().flush();
    let mut s = String::new();
    let _ = io::stdin().read_line(&mut s);
    let s = s.trim().trim_matches('"').trim();
    if s.is_empty() { default.to_string() } else { s.to_string() }
}

fn wizard(cli: &mut Cli) -> Result<()> {
    println!("compressor — split and/or compress videos (needs ffmpeg)\n");
    let path = prompt("Path to a video file or a folder (you can drag it here)", "");
    if path.is_empty() {
        bail!("no path given");
    }
    cli.input = Some(PathBuf::from(path));

    println!("\n  1) Compress only\n  2) Split only (into parts, no quality loss)\n  3) Compress, then split if still too big");
    cli.mode = match prompt("What do you want to do?", "3").as_str() {
        "1" => Mode::Compress,
        "2" => Mode::Split,
        _ => Mode::Both,
    };
    if cli.mode != Mode::Compress {
        cli.max_size = util::parse_size(&prompt("Max size per part", "50MB"))?;
    }
    if cli.input.as_ref().is_some_and(|p| p.is_dir()) {
        cli.recursive = prompt("Include sub-folders? (y/n)", "n").to_ascii_lowercase().starts_with('y');
    }
    if cli.mode != Mode::Split {
        cli.encoder = match prompt("Encoder: cpu (best quality) or auto (try GPU, faster)", "cpu").as_str() {
            "auto" => cli::EncoderKind::Auto,
            _ => cli::EncoderKind::Cpu,
        };
    }
    println!();
    Ok(())
}
