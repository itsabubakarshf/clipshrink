// Release builds on Windows are console apps (so the CLI prints output); the GUI hides the console itself.
mod cli;
mod encode;
mod ffmpeg;
mod gui;
mod job;
mod process;
mod util;

use std::process::exit;
use std::time::Duration;

use clap::Parser;
use indicatif::{ProgressBar, ProgressStyle};

use cli::Cli;
use ffmpeg::Ui;
use job::Job;
use util::fmt_size;

struct CliUi {
    pb: ProgressBar,
}

impl CliUi {
    fn new() -> Self {
        let pb = ProgressBar::new(1000);
        pb.set_style(
            ProgressStyle::with_template("  {msg:<26} [{bar:30.cyan/blue}] {percent:>3}%  {elapsed_precise}")
                .unwrap()
                .progress_chars("=> "),
        );
        pb.enable_steady_tick(Duration::from_millis(200));
        CliUi { pb }
    }
}

impl Ui for CliUi {
    fn progress(&self, f: f64) {
        self.pb.set_position((f.clamp(0.0, 1.0) * 1000.0) as u64);
    }
    fn status(&self, m: &str) {
        self.pb.set_message(m.to_string());
    }
    fn log(&self, m: &str) {
        if self.pb.is_hidden() {
            println!("{m}");
        } else {
            self.pb.println(m);
        }
    }
}

fn main() {
    if std::env::args().len() == 1 {
        exit(gui::run());
    }
    let cli = Cli::parse();
    let job = Job {
        input: cli.input.clone().unwrap_or_default(),
        mode: cli.mode,
        max_size: cli.max_size,
        output: cli.output,
        recursive: cli.recursive,
        overwrite: cli.overwrite,
        codec: cli.codec,
        encoder: cli.encoder,
        crf: cli.crf,
        reduce: cli.reduce,
        preset: cli.preset,
        height: cli.height,
        audio_bitrate: cli.audio_bitrate,
        ffmpeg: cli.ffmpeg,
    };
    if cli.input.is_none() {
        eprintln!("error: give a video file or folder (or run with no arguments to open the window)");
        exit(2);
    }
    let ui = CliUi::new();
    let code = match job::run(&job, &ui) {
        Ok(s) => {
            ui.pb.finish_and_clear();
            println!(
                "\nFinished {} file(s), {} failed. Total {} -> {}. Output: {}",
                s.files, s.failed, fmt_size(s.total_in), fmt_size(s.total_out), s.out_root.display()
            );
            i32::from(s.failed > 0)
        }
        Err(e) => {
            ui.pb.finish_and_clear();
            eprintln!("error: {e:#}");
            2
        }
    };
    exit(code);
}
