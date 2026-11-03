//! Locating ffmpeg/ffprobe, probing media and running ffmpeg with progress.

use std::ffi::{OsStr, OsString};
use std::io::{BufRead, BufReader, Read};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use anyhow::{anyhow, bail, Context, Result};
use indicatif::ProgressBar;

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
    Command::new(path)
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
            candidates.push((PathBuf::from("ffmpeg"), PathBuf::from("ffprobe")));
        }

        for (ffmpeg, ffprobe) in candidates {
            if works(&ffmpeg) && works(&ffprobe) {
                return Ok(Tools { ffmpeg, ffprobe });
            }
        }
        bail!(
            "ffmpeg/ffprobe not found.\n\
             Install it (Windows: `winget install Gyan.FFmpeg`), put ffmpeg.exe and ffprobe.exe \
             next to compressor.exe, or pass --ffmpeg <path>."
        )
    }

    pub fn probe(&self, path: &Path) -> Result<MediaInfo> {
        let out = Command::new(&self.ffprobe)
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
        })
    }

    /// Run ffmpeg, updating `pb` (length is in milliseconds of media time).
    pub fn run(&self, args: &Args, duration: f64, pb: &ProgressBar) -> Result<()> {
        pb.set_length((duration * 1000.0) as u64);
        pb.set_position(0);
        let mut child = Command::new(&self.ffmpeg)
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
                pb.set_position((us.max(0) / 1000) as u64);
            }
        }

        let status = child.wait()?;
        let err = err_thread.join().unwrap_or_default();
        if !status.success() {
            let tail: Vec<&str> = err.lines().rev().take(8).collect::<Vec<_>>().into_iter().rev().collect();
            bail!("ffmpeg failed ({status}):\n{}", tail.join("\n"));
        }
        pb.set_position(pb.length().unwrap_or(0));
        Ok(())
    }

    /// Is this GPU encoder actually usable on this machine?
    fn encoder_works(&self, name: &str) -> bool {
        Command::new(&self.ffmpeg)
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

#[derive(Debug, Clone)]
pub struct MediaInfo {
    pub duration: f64,
    pub size: u64,
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
