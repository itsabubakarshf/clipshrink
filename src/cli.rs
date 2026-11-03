use std::path::PathBuf;

use clap::{Parser, ValueEnum};

use crate::util::parse_size;

#[derive(Clone, Copy, Debug, PartialEq, Eq, ValueEnum)]
pub enum Mode {
    /// Only re-encode to a smaller file (no splitting)
    Compress,
    /// Only cut into parts of at most --max-size (no re-encoding, lossless & fast)
    Split,
    /// Compress first, then split the result if it is still over --max-size
    Both,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, ValueEnum)]
pub enum Codec {
    /// H.264 — plays everywhere
    H264,
    /// H.265/HEVC — ~30-40% smaller at the same quality, slower to encode
    H265,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, ValueEnum)]
pub enum EncoderKind {
    /// Software encoding (libx264/libx265): best quality per byte
    Cpu,
    /// Try GPU encoders (NVIDIA, Intel, AMD), fall back to CPU
    Auto,
    /// NVIDIA NVENC
    Nvenc,
    /// Intel Quick Sync
    Qsv,
    /// AMD AMF
    Amf,
}

#[derive(Parser, Debug)]
#[command(
    version,
    about = "Split videos into size-limited parts and/or compress them (ffmpeg wrapper)",
    long_about = "Split videos into parts of at most --max-size and/or compress them.\n\
                  Give it a video file or a folder. Run with no arguments for an interactive prompt."
)]
pub struct Cli {
    /// Video file or folder of videos (omit for interactive mode)
    pub input: Option<PathBuf>,

    /// What to do
    #[arg(short, long, value_enum, default_value_t = Mode::Both)]
    pub mode: Mode,

    /// Maximum size of each part when splitting (e.g. 50MB, 1.5GB)
    #[arg(short = 's', long, default_value = "50MB", value_parser = parse_size)]
    pub max_size: u64,

    /// Output folder (default: "compressor_out" next to the input)
    #[arg(short, long)]
    pub output: Option<PathBuf>,

    /// Also process videos in sub-folders when the input is a folder
    #[arg(short, long)]
    pub recursive: bool,

    /// Video codec used when compressing
    #[arg(long, value_enum, default_value_t = Codec::H264)]
    pub codec: Codec,

    /// Quality (lower = better/bigger). Default 26 for H.264, 28 for H.265
    #[arg(long)]
    pub crf: Option<u8>,

    /// x264/x265 speed preset (ultrafast … veryslow). CPU encoder only
    #[arg(long, default_value = "medium")]
    pub preset: String,

    /// Downscale to at most this height in pixels (e.g. 720). Never upscales
    #[arg(long)]
    pub height: Option<u32>,

    /// Audio bitrate when compressing
    #[arg(long, default_value = "128k")]
    pub audio_bitrate: String,

    /// Encoder backend
    #[arg(long, value_enum, default_value_t = EncoderKind::Cpu)]
    pub encoder: EncoderKind,

    /// Replace existing output files instead of skipping them
    #[arg(long)]
    pub overwrite: bool,

    /// Path to ffmpeg (file or folder). Default: next to this exe, then PATH
    #[arg(long)]
    pub ffmpeg: Option<PathBuf>,
}
