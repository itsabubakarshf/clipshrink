//! Compress / split pipelines for a single video.

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};

use crate::cli::{EncoderKind, Mode};
use crate::encode::EncodeSettings;
use crate::ffmpeg::{Args, MediaInfo, Slice, Tools, Ui};
use crate::util::{fmt_size, parse_bitrate};

pub struct Options {
    pub mode: Mode,
    pub limit: u64,
    pub overwrite: bool,
    /// Fraction of the original size to remove (compress/both), e.g. 0.4
    pub reduce: Option<f64>,
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
    ui: &dyn Ui,
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
            ui.status(&format!("compressing {stem}"));
            let size = compress(tools, opts, input, &out, &info, ui)?;
            Ok(Outcome::Done { in_size: info.size, out_size: size, parts: 1 })
        }
        Mode::Split => {
            if info.size <= opts.limit {
                return Ok(Outcome::Skipped(format!("already {} (under the limit)", fmt_size(info.size))));
            }
            if !prepare_parts(out_dir, &out_stem, opts.overwrite)? {
                return Ok(Outcome::Skipped("parts already exist (use --overwrite)".into()));
            }
            ui.status(&format!("splitting {stem}"));
            let parts = split(tools, opts, input, &info, out_dir, &out_stem, &ext, ui)?;
            Ok(summary(&info, &parts))
        }
        Mode::Both => {
            let out = out_dir.join(format!("{out_stem}.mp4"));
            if (out.exists() || parts_exist(out_dir, &out_stem)) && !opts.overwrite {
                return Ok(Outcome::Skipped("output already exists (use --overwrite)".into()));
            }
            remove_parts(out_dir, &out_stem);
            ui.status(&format!("compressing {stem}"));
            let size = compress(tools, opts, input, &out, &info, ui)?;
            if size <= opts.limit {
                return Ok(Outcome::Done { in_size: info.size, out_size: size, parts: 1 });
            }
            // Still too big: cut the compressed file (stream copy, no second re-encode).
            ui.status(&format!("splitting {stem}"));
            let cinfo = tools.probe(&out)?;
            let parts = split(tools, opts, &out, &cinfo, out_dir, &out_stem, "mp4", ui);
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
    opts: &Options,
    input: &Path,
    output: &Path,
    info: &MediaInfo,
    ui: &dyn Ui,
) -> Result<u64> {
    let mut enc = opts.encode.clone();
    let mut two_pass = false;
    if let Some(r) = opts.reduce {
        let audio_bps = if info.has_audio { parse_bitrate(&enc.audio_bitrate).unwrap_or(128_000) } else { 0 };
        // 1.5% headroom for container overhead.
        let total_bps = info.size as f64 * (1.0 - r) * 0.985 * 8.0 / info.duration;
        let video_bps = (total_bps - audio_bps as f64).max(64_000.0) as u64;
        if total_bps - (audio_bps as f64) < 64_000.0 {
            ui.log("  note: that reduction is very aggressive for this video; quality will be poor and the result may not reach the target");
        }
        enc.bitrate = Some(video_bps);
        two_pass = enc.encoder == EncoderKind::Cpu; // exact size needs two passes on CPU
    }

    let tmp = output.with_extension("part.mp4");
    let log_prefix = std::env::temp_dir().join(format!("compressor-{}-{}", std::process::id(), output.file_stem().and_then(|s| s.to_str()).unwrap_or("x")));
    let cleanup = || {
        let _ = fs::remove_file(&tmp);
        remove_pass_logs(&log_prefix);
    };

    let attempt = |two_pass: bool| -> Result<()> {
        if two_pass {
            let pass1 = Args::new()
                .a("-i").a(input)
                .many(["-map", "0:v:0", "-an"])
                .many(enc.args().0)
                .many(["-pass:v", "1", "-passlogfile"]).a(&log_prefix)
                .many(["-f", "null", "-"]);
            ui.status("analysing (pass 1/2)");
            tools.run(&pass1, info.duration, &Slice { ui, lo: 0.0, hi: 0.4 })?;
        }
        let mut args = Args::new().a("-i").a(input).many(["-map", "0:v:0", "-map", "0:a?"]).many(enc.args().0);
        if two_pass {
            args = args.many(["-pass:v", "2", "-passlogfile"]).a(&log_prefix);
            ui.status("encoding (pass 2/2)");
        }
        args = args.many(["-movflags", "+faststart"]).a(&tmp);
        let (lo, hi) = if two_pass { (0.4, 1.0) } else { (0.0, 1.0) };
        tools.run(&args, info.duration, &Slice { ui, lo, hi })
    };
    let mut result = attempt(two_pass);
    if result.is_err() && two_pass && !ui.cancelled() {
        // Two-pass is only for accuracy; if this ffmpeg build can't do it, fall back to one pass.
        ui.log("  note: two-pass encoding failed on this system, using single pass (size will be less exact)");
        remove_pass_logs(&log_prefix);
        let _ = fs::remove_file(&tmp);
        result = attempt(false);
    }
    remove_pass_logs(&log_prefix);
    if let Err(e) = result {
        cleanup();
        return Err(e);
    }
    let _ = fs::remove_file(output);
    fs::rename(&tmp, output)?;
    Ok(fs::metadata(output)?.len())
}

fn remove_pass_logs(prefix: &Path) {
    let (Some(dir), Some(name)) = (prefix.parent(), prefix.file_name().and_then(|n| n.to_str())) else { return };
    for e in fs::read_dir(dir).into_iter().flatten().flatten() {
        if e.file_name().to_string_lossy().starts_with(name) {
            let _ = fs::remove_file(e.path());
        }
    }
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
    ui: &dyn Ui,
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

        if let Err(e) = tools.run(&args, info.duration, ui) {
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
            ui.log(&format!(
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
