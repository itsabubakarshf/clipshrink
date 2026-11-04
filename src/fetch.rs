//! One-time automatic download of ffmpeg (Windows), stored per-user, no admin rights needed.

use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::{bail, Context, Result};

use crate::ffmpeg::{data_dir, Ui};

/// Tried in order. Both are Windows 64-bit builds containing bin/ffmpeg.exe and bin/ffprobe.exe.
const SOURCES: &[&str] = &[
    "https://www.gyan.dev/ffmpeg/builds/ffmpeg-release-essentials.zip",
    "https://github.com/BtbN/FFmpeg-Builds/releases/download/latest/ffmpeg-master-latest-win64-gpl.zip",
];

pub fn download_ffmpeg(ui: &dyn Ui) -> Result<PathBuf> {
    let dir = data_dir().context("cannot find a folder to store ffmpeg in")?;
    fs::create_dir_all(&dir)?;
    let zip_path = dir.join("ffmpeg-download.zip.part");

    let mut last_err = None;
    for url in SOURCES {
        ui.status("downloading ffmpeg");
        ui.log(&format!("  downloading ffmpeg from {url}"));
        match fetch(url, &zip_path, ui) {
            Ok(()) => {
                ui.status("unpacking ffmpeg");
                let r = extract(&zip_path, &dir);
                let _ = fs::remove_file(&zip_path);
                match r {
                    Ok(()) => {
                        ui.log(&format!("  ffmpeg installed to {}", dir.display()));
                        return Ok(dir);
                    }
                    Err(e) => last_err = Some(e),
                }
            }
            Err(e) => {
                let _ = fs::remove_file(&zip_path);
                if ui.cancelled() {
                    bail!("cancelled");
                }
                ui.log(&format!("  that source failed ({e:#}), trying the next one"));
                last_err = Some(e);
            }
        }
    }
    bail!(
        "could not download ffmpeg automatically ({}). Install it manually: see SETUP.md (winget install Gyan.FFmpeg)",
        last_err.map(|e| format!("{e:#}")).unwrap_or_default()
    )
}

fn fetch(url: &str, dest: &Path, ui: &dyn Ui) -> Result<()> {
    let agent = ureq::AgentBuilder::new().try_proxy_from_env(true).timeout_connect(Duration::from_secs(20)).timeout_read(Duration::from_secs(60)).build();
    let resp = agent.get(url).call().context("request failed")?;
    let total: Option<u64> = resp.header("Content-Length").and_then(|v| v.parse().ok());
    let mut reader = resp.into_reader();
    let mut out = File::create(dest)?;
    let mut buf = vec![0u8; 256 * 1024];
    let mut done = 0u64;
    loop {
        if ui.cancelled() {
            bail!("cancelled");
        }
        let n = reader.read(&mut buf).context("download interrupted")?;
        if n == 0 {
            break;
        }
        out.write_all(&buf[..n])?;
        done += n as u64;
        if let Some(t) = total {
            ui.progress(done as f64 / t as f64);
        }
    }
    out.flush()?;
    if done < 1_000_000 {
        bail!("download too small ({done} bytes)");
    }
    Ok(())
}

/// Pull `bin/ffmpeg.exe` and `bin/ffprobe.exe` out of the archive into `dir`.
fn extract(zip_path: &Path, dir: &Path) -> Result<()> {
    let mut zip = zip::ZipArchive::new(File::open(zip_path)?).context("not a valid zip file")?;
    let mut found = 0;
    for i in 0..zip.len() {
        let mut f = zip.by_index(i)?;
        let name = f.name().replace('\\', "/");
        let target = if name.ends_with("/bin/ffmpeg.exe") {
            "ffmpeg.exe"
        } else if name.ends_with("/bin/ffprobe.exe") {
            "ffprobe.exe"
        } else {
            continue;
        };
        let tmp = dir.join(format!("{target}.part"));
        std::io::copy(&mut f, &mut File::create(&tmp)?)?;
        fs::rename(&tmp, dir.join(target))?;
        found += 1;
    }
    if found < 2 {
        bail!("the archive did not contain ffmpeg.exe and ffprobe.exe");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Quiet;
    impl Ui for Quiet {
        fn progress(&self, _: f64) {}
        fn status(&self, _: &str) {}
        fn log(&self, m: &str) {
            eprintln!("{m}");
        }
    }

    /// Network test: `cargo test -- --ignored download`
    #[test]
    #[ignore]
    fn download_and_extract() {
        let d = std::env::temp_dir().join(format!("compressor-dl-{}", std::process::id()));
        std::env::set_var("COMPRESSOR_HOME", &d);
        let dir = download_ffmpeg(&Quiet).unwrap();
        for exe in ["ffmpeg.exe", "ffprobe.exe"] {
            let mut head = [0u8; 2];
            File::open(dir.join(exe)).unwrap().read_exact(&mut head).unwrap();
            assert_eq!(&head, b"MZ", "{exe} is not a Windows executable");
        }
        let _ = fs::remove_dir_all(d);
    }
}
