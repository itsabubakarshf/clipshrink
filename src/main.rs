// Release builds on Windows are console apps (so the CLI prints output); the GUI hides the console itself.
mod cli;
mod encode;
mod fetch;
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
use ffmpeg::{MissingChoice, Ui};
use job::Job;
use util::fmt_size;

struct CliUi {
    pb: ProgressBar,
    yes: bool,
}

impl CliUi {
    fn new(yes: bool) -> Self {
        let pb = ProgressBar::new(1000);
        pb.set_style(
            ProgressStyle::with_template("  {msg:<26} [{bar:30.cyan/blue}] {percent:>3}%  {elapsed_precise}")
                .unwrap()
                .progress_chars("=> "),
        );
        pb.enable_steady_tick(Duration::from_millis(200));
        CliUi { pb, yes }
    }
}

impl Ui for CliUi {
    fn progress(&self, f: f64) {
        self.pb.set_position((f.clamp(0.0, 1.0) * 1000.0) as u64);
    }
    fn status(&self, m: &str) {
        self.pb.set_message(m.to_string());
    }
    fn ffmpeg_missing(&self) -> MissingChoice {
        if !cfg!(windows) {
            return MissingChoice::Cancel;
        }
        if self.yes {
            return MissingChoice::Download;
        }
        if !std::io::IsTerminal::is_terminal(&std::io::stdin()) {
            return MissingChoice::Cancel;
        }
        println!("ffmpeg (the video engine this tool needs) was not found on this computer.");
        println!("It can be downloaded automatically (about 100-200 MB, one time only, no admin needed).");
        println!("(If you already have it somewhere, cancel and run again with --ffmpeg \"C:\\path\\to\\ffmpeg\\bin\".)");
        print!("Press Enter to download it, or Ctrl+C to cancel: ");
        let _ = std::io::Write::flush(&mut std::io::stdout());
        let _ = std::io::stdin().read_line(&mut String::new());
        MissingChoice::Download
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
    if cli.setup_ffmpeg {
        exit(setup_ffmpeg());
    }
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
    let yes = cli.yes;
    if cli.input.is_none() {
        eprintln!("error: give a video file or folder (or run with no arguments to open the window)");
        exit(2);
    }
    let ui = CliUi::new(yes);
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

/// `clipshrink --setup-ffmpeg`: used by the installer (and handy by hand) to get ffmpeg ready.
fn setup_ffmpeg() -> i32 {
    let ui = CliUi::new(true);
    let result = match ffmpeg::Tools::locate(None) {
        Ok(t) => {
            println!("ffmpeg is already available: {}", t.ffmpeg.display());
            Ok(())
        }
        Err(_) if cfg!(windows) => fetch::download_ffmpeg(&ui).and_then(|_| ffmpeg::Tools::locate(None)).map(|t| {
            ui.pb.finish_and_clear();
            println!("\nffmpeg is ready: {}", t.ffmpeg.display());
        }),
        Err(e) => Err(e),
    };
    ui.pb.finish_and_clear();
    match result {
        Ok(()) => {
            println!("All set. This window closes in a few seconds...");
            std::thread::sleep(Duration::from_secs(4));
            0
        }
        Err(e) => {
            eprintln!("\nCould not set up ffmpeg: {e:#}");
            eprintln!("You can also install it yourself: winget install Gyan.FFmpeg  (see SETUP.md)");
            print!("Press Enter to close...");
            let _ = std::io::Write::flush(&mut std::io::stdout());
            let _ = std::io::stdin().read_line(&mut String::new());
            1
        }
    }
}
