//! Compression settings -> ffmpeg arguments.

use crate::cli::{Codec, EncoderKind};
use crate::ffmpeg::{encoder_name, Args};

#[derive(Debug, Clone)]
pub struct EncodeSettings {
    pub codec: Codec,
    pub encoder: EncoderKind, // already resolved (never Auto)
    pub crf: u8,
    pub preset: String,
    pub height: Option<u32>,
    pub audio_bitrate: String,
}

impl EncodeSettings {
    pub fn default_crf(codec: Codec) -> u8 {
        match codec {
            Codec::H264 => 26,
            Codec::H265 => 28,
        }
    }

    /// Video + audio encoding arguments (without inputs/outputs).
    pub fn args(&self) -> Args {
        let name = encoder_name(self.encoder, self.codec);
        let q = self.crf.to_string();
        let mut a = Args::new();

        if let Some(h) = self.height {
            // Only ever downscale; keep width even.
            a = a.many(["-vf", &format!("scale=-2:min({h}\\,ih)")]);
        }

        a = match self.encoder {
            EncoderKind::Nvenc => a.many(["-c:v", name, "-preset", "p5", "-rc", "vbr", "-cq", &q, "-b:v", "0"]),
            EncoderKind::Qsv => a.many(["-c:v", name, "-global_quality", &q]),
            EncoderKind::Amf => a.many(["-c:v", name, "-rc", "cqp", "-qp_i", &q, "-qp_p", &q]),
            _ => a.many(["-c:v", name, "-preset", &self.preset, "-crf", &q]),
        };
        a = a.many(["-pix_fmt", "yuv420p"]);
        if self.codec == Codec::H265 {
            a = a.many(["-tag:v", "hvc1"]);
            if self.encoder == EncoderKind::Cpu {
                a = a.many(["-x265-params", "log-level=error"]);
            }
        }
        a.many(["-c:a", "aac", "-b:a", &self.audio_bitrate])
    }
}
