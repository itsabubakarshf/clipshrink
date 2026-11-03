//! End-to-end tests; need ffmpeg/ffprobe on PATH (skipped otherwise).

use std::path::{Path, PathBuf};
use std::process::Command;

fn have_ffmpeg() -> bool {
    Command::new("ffmpeg").arg("-version").output().is_ok() && Command::new("ffprobe").arg("-version").output().is_ok()
}

fn tmpdir(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("compressor-test-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

/// Noisy 40 s clip so the bitrate is high and the file is several MB.
fn make_video(path: &Path, keyint: u32) {
    let st = Command::new("ffmpeg")
        .args(["-v", "error", "-y", "-f", "lavfi", "-i", "testsrc2=size=640x360:rate=25:duration=40"])
        .args(["-f", "lavfi", "-i", "sine=frequency=440:duration=40"])
        .args(["-vf", "noise=alls=40:allf=t", "-c:v", "libx264", "-b:v", "1500k", "-g", &keyint.to_string(), "-c:a", "aac"])
        .arg(path)
        .status()
        .unwrap();
    assert!(st.success());
}

fn run(args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_compressor")).args(args).output().unwrap()
}

fn outputs(dir: &Path) -> Vec<(String, u64)> {
    let mut v: Vec<_> = std::fs::read_dir(dir)
        .unwrap()
        .flatten()
        .map(|e| (e.file_name().to_string_lossy().to_string(), e.metadata().unwrap().len()))
        .collect();
    v.sort();
    v
}

fn duration(p: &Path) -> f64 {
    let o = Command::new("ffprobe")
        .args(["-v", "error", "-show_entries", "format=duration", "-of", "default=nw=1:nk=1"])
        .arg(p)
        .output()
        .unwrap();
    String::from_utf8_lossy(&o.stdout).trim().parse().unwrap()
}

#[test]
fn split_respects_limit_and_keeps_all_content() {
    if !have_ffmpeg() {
        return;
    }
    let d = tmpdir("split");
    let video = d.join("clip.mp4");
    make_video(&video, 25);
    let total = std::fs::metadata(&video).unwrap().len();
    assert!(total > 3_000_000, "fixture too small: {total}");

    let out = run(&[video.to_str().unwrap(), "--mode", "split", "--max-size", "2MB"]);
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));

    let files = outputs(&d.join("compressor_out"));
    assert!(files.len() >= 2, "{files:?}");
    let mut dur = 0.0;
    for (name, size) in &files {
        assert!(*size <= 2_000_000, "{name} is {size} bytes");
        dur += duration(&d.join("compressor_out").join(name));
    }
    assert!((dur - 40.0).abs() < 1.5, "parts sum to {dur}s");
}

#[test]
fn split_falls_back_to_reencode_with_sparse_keyframes() {
    if !have_ffmpeg() {
        return;
    }
    let d = tmpdir("sparse");
    let video = d.join("clip.mp4");
    make_video(&video, 1000); // a single keyframe -> copy can't cut
    let out = run(&[video.to_str().unwrap(), "--mode", "split", "--max-size", "2MB", "--preset", "ultrafast"]);
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    for (name, size) in outputs(&d.join("compressor_out")) {
        assert!(size <= 2_000_000, "{name} is {size} bytes");
    }
}

#[test]
fn compress_only_makes_single_smaller_file() {
    if !have_ffmpeg() {
        return;
    }
    let d = tmpdir("compress");
    let video = d.join("clip.mp4");
    make_video(&video, 25);
    let before = std::fs::metadata(&video).unwrap().len();
    let out = run(&[video.to_str().unwrap(), "--mode", "compress", "--crf", "35", "--height", "240", "--preset", "ultrafast"]);
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    let files = outputs(&d.join("compressor_out"));
    assert_eq!(files.len(), 1, "{files:?}");
    assert!(files[0].1 < before);
}

#[test]
fn both_mode_on_folder() {
    if !have_ffmpeg() {
        return;
    }
    let d = tmpdir("both");
    make_video(&d.join("a.mp4"), 25);
    make_video(&d.join("b.mp4"), 25);
    let out = run(&[d.to_str().unwrap(), "--max-size", "500KB", "--crf", "32", "--height", "240", "--preset", "ultrafast"]);
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    let files = outputs(&d.join("compressor_out"));
    assert!(files.iter().any(|(n, _)| n.starts_with("a_part")), "{files:?}");
    for (name, size) in files {
        assert!(size <= 500_000, "{name} is {size}");
        assert!(!name.ends_with(".part.mp4"));
    }
    // Second run skips existing output.
    let again = run(&[d.to_str().unwrap(), "--max-size", "500KB", "--preset", "ultrafast"]);
    assert!(String::from_utf8_lossy(&again.stdout).contains("skipped"));
}
