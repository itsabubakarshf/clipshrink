//! A complete run (one file or a folder), independent of CLI/GUI.

use std::path::{Path, PathBuf};

use anyhow::{bail, Result};
use walkdir::WalkDir;

use crate::cli::{Codec, EncoderKind, Mode};
use crate::encode::EncodeSettings;
use crate::ffmpeg::{Tools, Ui};
use crate::process::{process_file, Options, Outcome};
use crate::util::fmt_size;

const VIDEO_EXTS: &[&str] = &["mp4", "mkv", "mov", "avi", "webm", "m4v", "wmv", "flv", "ts", "mts", "m2ts", "mpg", "mpeg", "3gp"];

#[derive(Clone, Debug)]
pub struct Job {
    pub input: PathBuf,
    pub mode: Mode,
    pub max_size: u64,
    pub output: Option<PathBuf>,
    pub recursive: bool,
    pub overwrite: bool,
    pub codec: Codec,
    pub encoder: EncoderKind,
    pub crf: Option<u8>,
    pub reduce: Option<f64>,
    pub preset: String,
    pub height: Option<u32>,
    pub audio_bitrate: String,
    pub ffmpeg: Option<PathBuf>,
}

pub struct Summary {
    pub files: usize,
    pub failed: usize,
    pub total_in: u64,
    pub total_out: u64,
    pub out_root: PathBuf,
    pub cancelled: bool,
}

pub fn run(job: &Job, ui: &dyn Ui) -> Result<Summary> {
    if !job.input.exists() {
        bail!("'{}' does not exist", job.input.display());
    }
    if job.reduce.is_some() && job.mode == Mode::Split {
        bail!("a size reduction only applies when compressing; pick 'compress' or 'both'");
    }
    let tools = Tools::locate(job.ffmpeg.as_deref())?;

    let needs_encode = job.mode != Mode::Split;
    let encoder = if needs_encode { tools.resolve_encoder(job.encoder, job.codec)? } else { EncoderKind::Cpu };
    let opts = Options {
        mode: job.mode,
        limit: job.max_size,
        overwrite: job.overwrite,
        reduce: job.reduce,
        encode: EncodeSettings {
            codec: job.codec,
            encoder,
            crf: job.crf.unwrap_or_else(|| EncodeSettings::default_crf(job.codec)),
            preset: job.preset.clone(),
            height: job.height,
            audio_bitrate: job.audio_bitrate.clone(),
            bitrate: None,
        },
    };

    let base = if job.input.is_dir() { job.input.clone() } else { job.input.parent().map(Path::to_path_buf).unwrap_or_default() };
    let out_root = job.output.clone().unwrap_or_else(|| base.join("compressor_out"));
    let files = collect_videos(&job.input, job.recursive, &out_root);
    if files.is_empty() {
        bail!(
            "no video files found in '{}'{}",
            job.input.display(),
            if job.input.is_dir() && !job.recursive { " (tick 'include sub-folders' / use --recursive to search deeper)" } else { "" }
        );
    }

    let what = match (job.mode, job.reduce) {
        (Mode::Split, _) => format!("split into parts of at most {}", fmt_size(job.max_size)),
        (Mode::Compress, Some(r)) => format!("compress, {:.0}% smaller", r * 100.0),
        (Mode::Compress, None) => "compress".to_string(),
        (Mode::Both, Some(r)) => format!("compress {:.0}% smaller, then split into parts of at most {}", r * 100.0, fmt_size(job.max_size)),
        (Mode::Both, None) => format!("compress, then split into parts of at most {}", fmt_size(job.max_size)),
    };
    ui.log(&format!("Plan: {what}. Output: {}", out_root.display()));

    let mut sum = Summary { files: files.len(), failed: 0, total_in: 0, total_out: 0, out_root: out_root.clone(), cancelled: false };
    for (i, file) in files.iter().enumerate() {
        if ui.cancelled() {
            sum.cancelled = true;
            break;
        }
        let rel_dir = file.strip_prefix(&base).ok().and_then(|r| r.parent()).unwrap_or(Path::new(""));
        let out_dir = out_root.join(rel_dir);
        let name = file.file_name().unwrap().to_string_lossy().to_string();
        ui.file_start(i + 1, files.len(), &name);
        ui.log(&format!("[{}/{}] {name}", i + 1, files.len()));

        match process_file(&tools, &opts, file, &out_dir, ui) {
            Ok(Outcome::Done { in_size, out_size, parts }) => {
                sum.total_in += in_size;
                sum.total_out += out_size;
                let delta = 100.0 * (1.0 - out_size as f64 / in_size as f64);
                ui.log(&format!(
                    "  done: {} -> {} ({}{:.0}%){}",
                    fmt_size(in_size),
                    fmt_size(out_size),
                    if delta >= 0.0 { "-" } else { "+" },
                    delta.abs(),
                    if parts > 1 { format!(", {parts} parts") } else { String::new() }
                ));
            }
            Ok(Outcome::Skipped(why)) => ui.log(&format!("  skipped: {why}")),
            Err(e) if ui.cancelled() => {
                let _ = e;
                sum.cancelled = true;
                ui.log("  cancelled");
                break;
            }
            Err(e) => {
                sum.failed += 1;
                ui.log(&format!("  FAILED: {e:#}"));
            }
        }
    }
    Ok(sum)
}

fn collect_videos(input: &Path, recursive: bool, out_root: &Path) -> Vec<PathBuf> {
    let is_video = |p: &Path| p.extension().and_then(|e| e.to_str()).map(|e| VIDEO_EXTS.contains(&e.to_ascii_lowercase().as_str())).unwrap_or(false);
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
        .filter(|p| !p.to_string_lossy().ends_with(".part.mp4"))
        .collect();
    v.sort();
    v
}
