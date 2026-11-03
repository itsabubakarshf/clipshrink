//! Compress / split pipelines for a single video.

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use indicatif::ProgressBar;

use crate::cli::Mode;
use crate::encode::EncodeSettings;
use crate::ffmpeg::{Args, MediaInfo, Tools};
use crate::util::fmt_size;

pub struct Options {
    pub mode: Mode,
    pub limit: u64,
    pub overwrite: bool,
    pub encode: EncodeSettings,
}

pub enum Outcome {
    Done { in_size: u64, out_size: u64, parts: usize },
    Skipped(String),
}

/// Fraction of the limit we aim for, leaving room for keyframe overshoot.
const TARGET_FILL: f64 = 0.92;
const MAX_ATTEMPTS: usize = 8;

pub fn process_file(
    tools: &Tools,
    opts: &Options,
    input: &Path,
    out_dir: &Path,
    pb: &ProgressBar,
) -> Result<Outcome> {
    let info = tools.probe(input)?;
    fs::create_dir_all(out_dir)?;

    let stem = input.file_stem().and_then(|s| s.to_str()).context("bad file name")?.to_string();
    let ext = input.extension().and_then(|s| s.to_str()).unwrap_or("mp4").to_ascii_lowercase();

    // Never overwrite the source if the output folder is the source folder.
    let same_dir = match (fs::canonicalize(out_dir), input.parent().map(fs::canonicalize)) {
        (Ok(a), Some(Ok(b))) => a == b,
        _ => false,
    };
    let out_stem = if same_dir { format!("{stem}_compressed") } else { stem.clone() };

    match opts.mode {
        Mode::Compress => {
            let out = out_dir.join(format!("{out_stem}.mp4"));
            if out.exists() && !opts.overwrite {
                return Ok(Outcome::Skipped(format!("{} already exists (use --overwrite)", out.display())));
            }
            pb.set_message(format!("compressing {stem}"));
            let size = compress(tools, &opts.encode, input, &out, &info, pb)?;
            Ok(Outcome::Done { in_size: info.size, out_size: size, parts: 1 })
        }
        Mode::Split => {
            if info.size <= opts.limit {
                return Ok(Outcome::Skipped(format!("already {} (under the limit)", fmt_size(info.size))));
            }
            if !prepare_parts(out_dir, &out_stem, opts.overwrite)? {
                return Ok(Outcome::Skipped("parts already exist (use --overwrite)".into()));
            }
            pb.set_message(format!("splitting {stem}"));
            let parts = split(tools, opts, input, &info, out_dir, &out_stem, &ext, pb)?;
            Ok(summary(&info, &parts))
        }
        Mode::Both => {
            let out = out_dir.join(format!("{out_stem}.mp4"));
            if (out.exists() || parts_exist(out_dir, &out_stem)) && !opts.overwrite {
                return Ok(Outcome::Skipped("output already exists (use --overwrite)".into()));
            }
            remove_parts(out_dir, &out_stem);
            pb.set_message(format!("compressing {stem}"));
            let size = compress(tools, &opts.encode, input, &out, &info, pb)?;
            if size <= opts.limit {
                return Ok(Outcome::Done { in_size: info.size, out_size: size, parts: 1 });
            }
            // Still too big: cut the compressed file (stream copy, no second re-encode).
            pb.set_message(format!("splitting {stem}"));
            let cinfo = tools.probe(&out)?;
            let parts = split(tools, opts, &out, &cinfo, out_dir, &out_stem, "mp4", pb);
            let _ = fs::remove_file(&out);
            Ok(summary(&info, &parts?))
        }
    }
}

fn summary(info: &MediaInfo, parts: &[PathBuf]) -> Outcome {
    let out_size = parts.iter().filter_map(|p| fs::metadata(p).ok()).map(|m| m.len()).sum();
    Outcome::Done { in_size: info.size, out_size, parts: parts.len() }
}

fn compress(
    tools: &Tools,
    enc: &EncodeSettings,
    input: &Path,
    output: &Path,
    info: &MediaInfo,
    pb: &ProgressBar,
) -> Result<u64> {
    let tmp = output.with_extension("part.mp4");
    let args = Args::new()
        .a("-i").a(input)
        .many(["-map", "0:v:0", "-map", "0:a?"])
        .many(enc.args().0)
        .many(["-movflags", "+faststart"])
        .a(&tmp);
    if let Err(e) = tools.run(&args, info.duration, pb) {
        let _ = fs::remove_file(&tmp);
        return Err(e);
    }
    let _ = fs::remove_file(output);
    fs::rename(&tmp, output)?;
    Ok(fs::metadata(output)?.len())
}

// ---------- splitting ----------

/// `name_part001.ext` style files belonging to `stem`.
fn part_files(dir: &Path, stem: &str) -> Vec<PathBuf> {
    let prefix = format!("{stem}_part");
    let mut v: Vec<PathBuf> = fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .filter(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .and_then(|n| n.strip_prefix(&prefix))
                .map(|rest| {
                    let digits = rest.chars().take_while(|c| c.is_ascii_digit()).count();
                    digits >= 3 && rest[digits..].starts_with('.')
                })
                .unwrap_or(false)
        })
        .collect();
    v.sort();
    v
}

fn parts_exist(dir: &Path, stem: &str) -> bool {
    !part_files(dir, stem).is_empty()
}

fn remove_parts(dir: &Path, stem: &str) {
    for p in part_files(dir, stem) {
        let _ = fs::remove_file(p);
    }
}

/// Returns false if parts exist and we must not touch them.
fn prepare_parts(dir: &Path, stem: &str, overwrite: bool) -> Result<bool> {
    if parts_exist(dir, stem) {
        if !overwrite {
            return Ok(false);
        }
        remove_parts(dir, stem);
    }
    Ok(true)
}

/// ffmpeg treats `%` in the output pattern specially.
fn escape_pct(s: &str) -> String {
    s.replace('%', "%%")
}

#[derive(PartialEq, Clone, Copy)]
enum Cut {
    /// Stream copy: lossless and very fast, but cuts only on keyframes.
    Copy,
    /// Re-encode with forced keyframes: exact cuts, used if copy can't hit the limit.
    Reencode,
}

#[allow(clippy::too_many_arguments)]
fn split(
    tools: &Tools,
    opts: &Options,
    input: &Path,
    info: &MediaInfo,
    out_dir: &Path,
    stem: &str,
    ext: &str,
    pb: &ProgressBar,
) -> Result<Vec<PathBuf>> {
    let limit = opts.limit as f64;
    let n = (info.size as f64 / (limit * TARGET_FILL)).ceil().max(2.0);
    let mut seg_time = info.duration / n * 0.97;
    let mut cut = Cut::Copy;

    for attempt in 1..=MAX_ATTEMPTS {
        remove_parts(out_dir, stem);
        let out_ext = if cut == Cut::Copy { ext } else { "mp4" };
        let pattern = out_dir.join(format!("{}_part%03d.{out_ext}", escape_pct(stem)));

        let mut args = Args::new().a("-i").a(input).many(["-map", "0:v:0", "-map", "0:a?"]);
        args = match cut {
            Cut::Copy => args.many(["-c", "copy"]),
            Cut::Reencode => args
                .many(opts.encode.args().0)
                .many(["-force_key_frames", &format!("expr:gte(t,n_forced*{seg_time:.3})")]),
        };
        args = args.many(["-f", "segment", "-segment_time", &format!("{seg_time:.3}")]);
        args = args.many(["-segment_start_number", "1", "-reset_timestamps", "1"]);
        if matches!(out_ext, "mp4" | "mov" | "m4v") {
            args = args.many(["-segment_format_options", "movflags=+faststart"]);
        }
        args = args.a(&pattern);

        if let Err(e) = tools.run(&args, info.duration, pb) {
            remove_parts(out_dir, stem);
            return Err(e);
        }

        let parts = part_files(out_dir, stem);
        let max = parts.iter().filter_map(|p| fs::metadata(p).ok()).map(|m| m.len()).max().unwrap_or(0);
        if parts.is_empty() {
            bail!("ffmpeg produced no output parts");
        }
        if max <= opts.limit {
            return Ok(parts);
        }

        // Too big: shrink the segment length proportionally and retry.
        seg_time *= (limit / max as f64) * TARGET_FILL;
        if cut == Cut::Copy && (attempt >= 3 || seg_time < 2.0) {
            pb.println(format!(
                "  note: this video can't be cut small enough without re-encoding (long gaps between keyframes); \
                 re-encoding to hit {} per part",
                fmt_size(opts.limit)
            ));
            cut = Cut::Reencode;
            // Re-encoded size is a different regime; restart from a fresh estimate.
            seg_time = info.duration / n * 0.9;
        }
        if seg_time < 1.0 {
            remove_parts(out_dir, stem);
            bail!("could not fit parts under {} (bitrate too high for such a small limit)", fmt_size(opts.limit));
        }
    }
    remove_parts(out_dir, stem);
    bail!("could not get all parts under {} after {MAX_ATTEMPTS} attempts", fmt_size(opts.limit))
}
