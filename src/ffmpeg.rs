//! Locating ffmpeg/ffprobe, probing media and running ffmpeg with progress.

use std::ffi::{OsStr, OsString};
use std::io::{BufRead, BufReader, Read};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

/// Every child process goes through here so no console window flashes up on Windows.
pub fn command(program: impl AsRef<OsStr>) -> Command {
    #[allow(unused_mut)]
    let mut c = Command::new(program);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        c.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
    }
    c
}

/// Where an automatically downloaded ffmpeg lives (per user, no admin needed).
pub fn data_dir() -> Option<PathBuf> {
    if let Some(d) = std::env::var_os("COMPRESSOR_HOME") {
        return Some(PathBuf::from(d));
    }
    let var = |k: &str| std::env::var_os(k).map(PathBuf::from);
    if cfg!(windows) {
        var("LOCALAPPDATA").map(|p| p.join("compressor").join("ffmpeg"))
    } else if cfg!(target_os = "macos") {
        var("HOME").map(|p| p.join("Library/Application Support/compressor/ffmpeg"))
    } else {
        var("XDG_DATA_HOME").or_else(|| var("HOME").map(|h| h.join(".local/share"))).map(|p| p.join("compressor/ffmpeg"))
    }
}

/// A folder (or ffmpeg file) the user pointed us to once; remembered between runs.
fn remembered_dir() -> Option<PathBuf> {
    let txt = std::fs::read_to_string(data_dir()?.join("ffmpeg-path.txt")).ok()?;
    let p = PathBuf::from(txt.trim());
    Some(if p.is_file() { p.parent()?.to_path_buf() } else { p })
}

#[cfg_attr(not(windows), allow(dead_code))]
pub fn remember_ffmpeg(p: &Path) -> Result<()> {
    let dir = data_dir().ok_or_else(|| anyhow!("no place to save settings"))?;
    std::fs::create_dir_all(&dir)?;
    std::fs::write(dir.join("ffmpeg-path.txt"), p.to_string_lossy().as_bytes())?;
    Ok(())
}

/// Folders called "*ffmpeg*" (e.g. an unzipped download) in the usual places: look inside for bin/ffmpeg.exe.
fn scan_ffmpeg_named_folders() -> Vec<PathBuf> {
    let var = |k: &str| std::env::var_os(k).map(PathBuf::from);
    let mut bases: Vec<PathBuf> = Vec::new();
    if let Some(u) = var("USERPROFILE").or_else(|| var("HOME")) {
        for sub in ["Downloads", "Desktop", "Documents", ""] {
            bases.push(u.join(sub));
        }
    }
    if cfg!(windows) {
        for d in ["C:\\", "C:\\Tools", "C:\\Program Files", "D:\\"] {
            bases.push(PathBuf::from(d));
        }
    }
    scan_bases(&bases)
}

fn scan_bases(bases: &[PathBuf]) -> Vec<PathBuf> {
    let exe_name = exe("ffmpeg");
    let mut found = Vec::new();
    for base in bases {
        for e in std::fs::read_dir(base).into_iter().flatten().flatten() {
            let name = e.file_name().to_string_lossy().to_ascii_lowercase();
            if !name.contains("ffmpeg") || !e.path().is_dir() {
                continue;
            }
            for f in walkdir::WalkDir::new(e.path()).max_depth(4).into_iter().flatten() {
                if f.file_name().to_string_lossy() == exe_name {
                    if let Some(p) = f.path().parent() {
                        found.push(p.to_path_buf());
                    }
                }
            }
        }
    }
    found
}

/// Raised when no ffmpeg can be found anywhere.
#[derive(Debug)]
pub struct FfmpegMissing;
impl std::fmt::Display for FfmpegMissing {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "ffmpeg/ffprobe not found.\nInstall it (Windows: `winget install Gyan.FFmpeg`; Ubuntu: `sudo apt install ffmpeg`; Mac: `brew install ffmpeg`), \
             put ffmpeg and ffprobe next to this program, or pass --ffmpeg <path>."
        )
    }
}
impl std::error::Error for FfmpegMissing {}

/// Folders where ffmpeg commonly lives even when it is not on PATH (e.g. right after winget install).
fn well_known_dirs() -> Vec<PathBuf> {
    let var = |k: &str| std::env::var_os(k).map(PathBuf::from);
    let mut v: Vec<PathBuf> = Vec::new();
    if cfg!(windows) {
        if let Some(l) = var("LOCALAPPDATA") {
            v.push(l.join("Microsoft\\WinGet\\Links"));
            let pk = l.join("Microsoft\\WinGet\\Packages");
            for e in walkdir::WalkDir::new(&pk).max_depth(4).into_iter().flatten() {
                if e.file_name() == "ffmpeg.exe" {
                    if let Some(p) = e.path().parent() {
                        v.push(p.to_path_buf());
                    }
                }
            }
        }
        if let Some(u) = var("USERPROFILE") {
            v.push(u.join("scoop\\shims"));
            v.push(u.join("scoop\\apps\\ffmpeg\\current\\bin"));
        }
        for d in ["C:\\ffmpeg\\bin", "C:\\Program Files\\ffmpeg\\bin", "C:\\ProgramData\\chocolatey\\bin", "C:\\Tools\\ffmpeg\\bin"] {
            v.push(PathBuf::from(d));
        }
    } else {
        for d in ["/usr/local/bin", "/opt/homebrew/bin", "/usr/bin", "/snap/bin"] {
            v.push(PathBuf::from(d));
        }
    }
    v
}

use anyhow::{anyhow, bail, Context, Result};

use crate::cli::{Codec, EncoderKind};

pub struct Tools {
    pub ffmpeg: PathBuf,
    pub ffprobe: PathBuf,
}

fn exe(name: &str) -> String {
    if cfg!(windows) {
        format!("{name}.exe")
    } else {
        name.to_string()
    }
}

fn works(path: &Path) -> bool {
    command(path)
        .arg("-version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

impl Tools {
    /// Search order: --ffmpeg, folder of this executable, PATH.
    pub fn locate(custom: Option<&Path>) -> Result<Self> {
        let mut candidates: Vec<(PathBuf, PathBuf)> = Vec::new();
        let pair_in = |dir: &Path| (dir.join(exe("ffmpeg")), dir.join(exe("ffprobe")));

        if let Some(c) = custom {
            if c.is_dir() {
                candidates.push(pair_in(c));
                candidates.push(pair_in(&c.join("bin")));
            } else {
                let dir = c.parent().map(Path::to_path_buf).unwrap_or_default();
                candidates.push((c.to_path_buf(), dir.join(exe("ffprobe"))));
            }
        } else {
            if let Some(dir) = std::env::current_exe().ok().and_then(|p| p.parent().map(Path::to_path_buf)) {
                candidates.push(pair_in(&dir));
            }
            if let Some(d) = remembered_dir() {
                candidates.push(pair_in(&d));
                candidates.push(pair_in(&d.join("bin")));
            }
            if let Some(d) = data_dir() {
                candidates.push(pair_in(&d));
            }
            candidates.push((PathBuf::from("ffmpeg"), PathBuf::from("ffprobe")));
            for d in well_known_dirs() {
                candidates.push(pair_in(&d));
            }
            for d in scan_ffmpeg_named_folders() {
                candidates.push(pair_in(&d));
            }
        }

        for (ffmpeg, ffprobe) in candidates {
            if works(&ffmpeg) && works(&ffprobe) {
                return Ok(Tools { ffmpeg, ffprobe });
            }
        }
        Err(FfmpegMissing.into())
    }

    pub fn probe(&self, path: &Path) -> Result<MediaInfo> {
        let out = command(&self.ffprobe)
            .args(["-v", "error", "-print_format", "json", "-show_format", "-show_streams"])
            .arg(path)
            .output()
            .context("failed to run ffprobe")?;
        if !out.status.success() {
            bail!("ffprobe could not read the file: {}", String::from_utf8_lossy(&out.stderr).trim());
        }
        let v: serde_json::Value = serde_json::from_slice(&out.stdout)?;
        let streams = v["streams"].as_array().cloned().unwrap_or_default();
        let video = streams
            .iter()
            .find(|s| s["codec_type"] == "video" && s["disposition"]["attached_pic"] != 1)
            .ok_or_else(|| anyhow!("no video stream"))?;
        let duration = v["format"]["duration"]
            .as_str()
            .and_then(|d| d.parse::<f64>().ok())
            .or_else(|| video["duration"].as_str().and_then(|d| d.parse().ok()))
            .filter(|d| *d > 0.0)
            .ok_or_else(|| anyhow!("could not determine duration"))?;
        Ok(MediaInfo {
            duration,
            size: std::fs::metadata(path)?.len(),
            has_audio: streams.iter().any(|s| s["codec_type"] == "audio"),
        })
    }

    /// Run ffmpeg, reporting progress (0..1 of `duration`) to `ui`.
    pub fn run(&self, args: &Args, duration: f64, ui: &dyn Ui) -> Result<()> {
        ui.progress(0.0);
        let mut child = command(&self.ffmpeg)
            .args(["-hide_banner", "-nostdin", "-y", "-loglevel", "error", "-nostats", "-progress", "pipe:1"])
            .args(&args.0)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .context("failed to start ffmpeg")?;

        let mut stderr = child.stderr.take().unwrap();
        let err_thread = std::thread::spawn(move || {
            let mut s = String::new();
            let _ = stderr.read_to_string(&mut s);
            s
        });

        for line in BufReader::new(child.stdout.take().unwrap()).lines().map_while(|l| l.ok()) {
            // Despite its name, out_time_ms is in microseconds too.
            if let Some(us) = line
                .strip_prefix("out_time_us=")
                .or_else(|| line.strip_prefix("out_time_ms="))
                .and_then(|v| v.trim().parse::<i64>().ok())
            {
                ui.progress(us.max(0) as f64 / 1e6 / duration);
            }
            if ui.cancelled() {
                let _ = child.kill();
                let _ = child.wait();
                bail!("cancelled");
            }
        }

        let status = child.wait()?;
        let err = err_thread.join().unwrap_or_default();
        if !status.success() {
            let tail: Vec<&str> = err.lines().rev().take(8).collect::<Vec<_>>().into_iter().rev().collect();
            bail!("ffmpeg failed ({status}):\n{}", tail.join("\n"));
        }
        ui.progress(1.0);
        Ok(())
    }

    /// Is this GPU encoder actually usable on this machine?
    fn encoder_works(&self, name: &str) -> bool {
        command(&self.ffmpeg)
            .args(["-hide_banner", "-loglevel", "error", "-f", "lavfi", "-i", "color=c=black:s=320x240:d=0.2"])
            .args(["-c:v", name, "-f", "null", "-"])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
    }

    /// Turn `Auto` into a concrete backend; explicit GPU choices are verified.
    pub fn resolve_encoder(&self, kind: EncoderKind, codec: Codec) -> Result<EncoderKind> {
        match kind {
            EncoderKind::Cpu => Ok(EncoderKind::Cpu),
            EncoderKind::Auto => {
                for k in [EncoderKind::Nvenc, EncoderKind::Qsv, EncoderKind::Amf] {
                    if self.encoder_works(encoder_name(k, codec)) {
                        return Ok(k);
                    }
                }
                Ok(EncoderKind::Cpu)
            }
            k => {
                if self.encoder_works(encoder_name(k, codec)) {
                    Ok(k)
                } else {
                    bail!("encoder {} is not available on this machine; use --encoder cpu or auto", encoder_name(k, codec))
                }
            }
        }
    }
}

pub fn encoder_name(kind: EncoderKind, codec: Codec) -> &'static str {
    match (kind, codec) {
        (EncoderKind::Nvenc, Codec::H264) => "h264_nvenc",
        (EncoderKind::Nvenc, Codec::H265) => "hevc_nvenc",
        (EncoderKind::Qsv, Codec::H264) => "h264_qsv",
        (EncoderKind::Qsv, Codec::H265) => "hevc_qsv",
        (EncoderKind::Amf, Codec::H264) => "h264_amf",
        (EncoderKind::Amf, Codec::H265) => "hevc_amf",
        (_, Codec::H264) => "libx264",
        (_, Codec::H265) => "libx265",
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(not(windows), allow(dead_code))]
pub enum MissingChoice {
    /// Download it automatically (Windows)
    Download,
    /// The user pointed us to an existing copy; search again
    Retry,
    Cancel,
}

/// Where progress and messages go (terminal or GUI).
pub trait Ui: Sync {
    /// Progress of the current step, 0.0..=1.0
    fn progress(&self, frac: f64);
    /// Short description of the current step
    fn status(&self, msg: &str);
    /// A line for the log
    fn log(&self, msg: &str);
    fn file_start(&self, _index: usize, _total: usize, _name: &str) {}
    fn cancelled(&self) -> bool {
        false
    }
    /// ffmpeg wasn't found: ask the user what to do.
    fn ffmpeg_missing(&self) -> MissingChoice {
        MissingChoice::Cancel
    }
}

/// Maps progress of a sub-step onto a slice [lo, hi] of the whole.
pub struct Slice<'a> {
    pub ui: &'a dyn Ui,
    pub lo: f64,
    pub hi: f64,
}

impl Ui for Slice<'_> {
    fn progress(&self, f: f64) {
        self.ui.progress(self.lo + (self.hi - self.lo) * f.clamp(0.0, 1.0));
    }
    fn status(&self, m: &str) {
        self.ui.status(m)
    }
    fn log(&self, m: &str) {
        self.ui.log(m)
    }
    fn cancelled(&self) -> bool {
        self.ui.cancelled()
    }
}

#[derive(Debug, Clone)]
pub struct MediaInfo {
    pub duration: f64,
    pub size: u64,
    pub has_audio: bool,
}

/// Tiny builder for ffmpeg argument lists (keeps paths as OsStrings).
#[derive(Default, Debug)]
pub struct Args(pub Vec<OsString>);

impl Args {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn a(mut self, x: impl AsRef<OsStr>) -> Self {
        self.0.push(x.as_ref().to_os_string());
        self
    }
    pub fn many<I, S>(mut self, it: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        for x in it {
            self.0.push(x.as_ref().to_os_string());
        }
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_unzipped_download_and_remembered_folder() {
        let root = std::env::temp_dir().join(format!("compressor-scan-{}", std::process::id()));
        let bin = root.join("Downloads/ffmpeg-2026-11-02-git-abc-essentials_build/bin");
        std::fs::create_dir_all(&bin).unwrap();
        std::fs::write(bin.join(exe("ffmpeg")), b"x").unwrap();
        std::fs::write(bin.join(exe("ffprobe")), b"x").unwrap();
        std::fs::create_dir_all(root.join("Downloads/holiday-videos/bin")).unwrap();
        std::fs::write(root.join("Downloads/holiday-videos/bin").join(exe("ffmpeg")), b"x").unwrap();

        let found = scan_bases(&[root.join("Downloads")]);
        assert_eq!(found, vec![bin.clone()]); // only folders with "ffmpeg" in the name

        std::env::set_var("COMPRESSOR_HOME", root.join("home"));
        remember_ffmpeg(&bin.join(exe("ffmpeg"))).unwrap(); // a file path is remembered as its folder
        assert_eq!(remembered_dir(), Some(bin));
        let _ = std::fs::remove_dir_all(root);
    }
}
