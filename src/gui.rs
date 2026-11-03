//! Simple desktop window (egui). Opened when the program is started without arguments.

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use eframe::egui;

use crate::cli::{Codec, EncoderKind, Mode};
use crate::ffmpeg::Ui;
use crate::job::{self, Job};
use crate::util::{fmt_size, parse_size};

#[derive(Default)]
struct RunState {
    running: bool,
    frac: f64,
    status: String,
    file: String,
    log: Vec<String>,
    finished: Option<String>,
    out_dir: Option<PathBuf>,
}

struct Shared {
    state: Mutex<RunState>,
    cancel: AtomicBool,
    ctx: egui::Context,
}

struct GuiUi(Arc<Shared>);

impl GuiUi {
    fn with(&self, f: impl FnOnce(&mut RunState)) {
        f(&mut self.0.state.lock().unwrap());
        self.0.ctx.request_repaint();
    }
}

impl Ui for GuiUi {
    fn progress(&self, f: f64) {
        self.with(|s| s.frac = f.clamp(0.0, 1.0));
    }
    fn status(&self, m: &str) {
        self.with(|s| s.status = m.to_string());
    }
    fn log(&self, m: &str) {
        self.with(|s| s.log.push(m.to_string()));
    }
    fn file_start(&self, i: usize, n: usize, name: &str) {
        self.with(|s| s.file = if n > 1 { format!("[{i}/{n}] {name}") } else { name.to_string() });
    }
    fn cancelled(&self) -> bool {
        self.0.cancel.load(Ordering::Relaxed)
    }
}

struct App {
    shared: Arc<Shared>,
    input: String,
    input_size: Option<u64>,
    mode: Mode,
    reduce_pct: u32,
    use_quality: bool,
    max_mb: f64,
    codec: Codec,
    gpu: bool,
    height: String,
    recursive: bool,
    overwrite: bool,
    output: String,
}

impl App {
    fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let mut style = (*cc.egui_ctx.global_style()).clone();
        style.spacing.item_spacing = egui::vec2(8.0, 8.0);
        style.spacing.button_padding = egui::vec2(12.0, 6.0);
        cc.egui_ctx.set_global_style(style);
        cc.egui_ctx.set_zoom_factor(1.15);
        App {
            shared: Arc::new(Shared { state: Mutex::default(), cancel: AtomicBool::new(false), ctx: cc.egui_ctx.clone() }),
            input: String::new(),
            input_size: None,
            mode: Mode::Both,
            reduce_pct: 40,
            use_quality: false,
            max_mb: 50.0,
            codec: Codec::H264,
            gpu: false,
            height: String::new(),
            recursive: false,
            overwrite: false,
            output: String::new(),
        }
    }

    fn set_input(&mut self, p: String) {
        let p = p.trim().trim_matches('"').to_string();
        self.input_size = std::fs::metadata(&p).ok().filter(|m| m.is_file()).map(|m| m.len());
        self.input = p;
    }

    fn running(&self) -> bool {
        self.shared.state.lock().unwrap().running
    }

    fn build_job(&self) -> Result<Job, String> {
        if self.input.trim().is_empty() {
            return Err("Choose a video or folder first.".into());
        }
        let height = match self.height.trim() {
            "" => None,
            h => Some(h.parse::<u32>().map_err(|_| "Max height must be a number like 720".to_string())?),
        };
        let max_size = parse_size(&format!("{}MB", self.max_mb)).map_err(|e| e.to_string())?;
        Ok(Job {
            input: PathBuf::from(self.input.trim()),
            mode: self.mode,
            max_size,
            output: (!self.output.trim().is_empty()).then(|| PathBuf::from(self.output.trim())),
            recursive: self.recursive,
            overwrite: self.overwrite,
            codec: self.codec,
            encoder: if self.gpu { EncoderKind::Auto } else { EncoderKind::Cpu },
            crf: None,
            reduce: (self.mode != Mode::Split && !self.use_quality).then(|| self.reduce_pct as f64 / 100.0),
            preset: "medium".into(),
            height,
            audio_bitrate: "128k".into(),
            ffmpeg: None,
        })
    }

    fn start(&mut self) {
        let job = match self.build_job() {
            Ok(j) => j,
            Err(e) => {
                self.shared.state.lock().unwrap().finished = Some(format!("Problem: {e}"));
                return;
            }
        };
        *self.shared.state.lock().unwrap() = RunState { running: true, status: "starting".into(), ..Default::default() };
        self.shared.cancel.store(false, Ordering::Relaxed);
        let shared = self.shared.clone();
        std::thread::spawn(move || {
            let ui = GuiUi(shared.clone());
            let res = job::run(&job, &ui);
            ui.with(|s| {
                s.running = false;
                s.frac = 1.0;
                match res {
                    Ok(sum) => {
                        s.out_dir = Some(sum.out_root.clone());
                        s.finished = Some(if sum.cancelled {
                            "Cancelled.".into()
                        } else if sum.failed > 0 {
                            format!("Finished with {} error(s) — see the log.", sum.failed)
                        } else {
                            format!("Done. {} → {}", fmt_size(sum.total_in), fmt_size(sum.total_out))
                        });
                    }
                    Err(e) => s.finished = Some(format!("Problem: {e:#}")),
                }
            });
        });
    }
}

fn open_folder(p: &std::path::Path) {
    let cmd = if cfg!(windows) { "explorer" } else if cfg!(target_os = "macos") { "open" } else { "xdg-open" };
    let _ = std::process::Command::new(cmd).arg(p).spawn();
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        // Drag & drop a file or folder onto the window.
        let dropped = ui.ctx().input(|i| i.raw.dropped_files.first().map(|f| f.path().to_path_buf()));
        if let Some(p) = dropped {
            self.set_input(p.display().to_string());
        }

        let running = self.running();
        egui::CentralPanel::default().show(ui, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| {
                ui.heading("Video Compressor");
                ui.label("Make videos smaller, or cut them into parts that fit an upload limit.");
                ui.separator();

                // 1. Source
                ui.strong("1. Choose a video or folder");
                ui.horizontal(|ui| {
                    let mut text = self.input.clone();
                    let resp = ui.add(egui::TextEdit::singleline(&mut text).desired_width(ui.available_width() - if cfg!(windows) { 250.0 } else { 0.0 }).hint_text("Drop a file here, or paste a path"));
                    if resp.changed() {
                        self.set_input(text);
                    }
                    #[cfg(windows)]
                    {
                        if ui.button("Choose file…").clicked() {
                            if let Some(p) = rfd::FileDialog::new().add_filter("Video", &["mp4", "mkv", "mov", "avi", "webm", "m4v", "wmv", "flv", "ts"]).pick_file() {
                                self.set_input(p.display().to_string());
                            }
                        }
                        if ui.button("Choose folder…").clicked() {
                            if let Some(p) = rfd::FileDialog::new().pick_folder() {
                                self.set_input(p.display().to_string());
                            }
                        }
                    }
                });
                if let Some(sz) = self.input_size {
                    ui.weak(format!("Size: {}", fmt_size(sz)));
                }

                // 2. What to do
                ui.add_space(4.0);
                ui.strong("2. What do you want to do?");
                ui.horizontal_wrapped(|ui| {
                    ui.selectable_value(&mut self.mode, Mode::Compress, "Compress");
                    ui.selectable_value(&mut self.mode, Mode::Split, "Split into parts");
                    ui.selectable_value(&mut self.mode, Mode::Both, "Compress, then split");
                });

                if self.mode != Mode::Split {
                    ui.add_space(4.0);
                    ui.strong("3. How much smaller?");
                    if self.use_quality {
                        ui.weak("Using fixed quality (set in Advanced). Untick it there to pick a percentage.");
                    } else {
                        ui.horizontal(|ui| {
                            ui.spacing_mut().slider_width = 280.0;
                            ui.add(egui::Slider::new(&mut self.reduce_pct, 10..=90).suffix("%  smaller").trailing_fill(true));
                        });
                        ui.horizontal(|ui| {
                            for p in [30, 40, 50, 60, 70] {
                                if ui.selectable_label(self.reduce_pct == p, format!("{p}%")).clicked() {
                                    self.reduce_pct = p;
                                }
                            }
                        });
                        if let Some(sz) = self.input_size {
                            let after = sz as f64 * (1.0 - self.reduce_pct as f64 / 100.0);
                            ui.label(format!("About {} → about {}", fmt_size(sz), fmt_size(after as u64)));
                        }
                    }
                }
                if self.mode != Mode::Compress {
                    ui.add_space(4.0);
                    ui.strong(if self.mode == Mode::Split { "3. Maximum size of each part" } else { "4. Maximum size of each part" });
                    ui.horizontal(|ui| {
                        ui.add(egui::DragValue::new(&mut self.max_mb).range(1.0..=100_000.0).speed(1.0).suffix(" MB"));
                        for p in [10.0, 25.0, 50.0, 100.0] {
                            if ui.selectable_label(self.max_mb == p, format!("{p} MB")).clicked() {
                                self.max_mb = p;
                            }
                        }
                    });
                }

                ui.add_space(2.0);
                egui::CollapsingHeader::new("Advanced").show(ui, |ui| {
                    ui.checkbox(&mut self.use_quality, "Use fixed quality instead of a size percentage");
                    ui.horizontal(|ui| {
                        ui.label("Codec");
                        ui.selectable_value(&mut self.codec, Codec::H264, "H.264 (most compatible)");
                        ui.selectable_value(&mut self.codec, Codec::H265, "H.265 (smaller, slower)");
                    });
                    ui.checkbox(&mut self.gpu, "Use graphics card if available (faster)");
                    ui.horizontal(|ui| {
                        ui.label("Max height (px)");
                        ui.add(egui::TextEdit::singleline(&mut self.height).desired_width(60.0).hint_text("e.g. 720"));
                    });
                    ui.checkbox(&mut self.recursive, "Include sub-folders (when a folder is chosen)");
                    ui.checkbox(&mut self.overwrite, "Replace existing results");
                    ui.horizontal(|ui| {
                        ui.label("Output folder");
                        ui.add(egui::TextEdit::singleline(&mut self.output).desired_width(ui.available_width()).hint_text("default: “compressor_out” next to the video"));
                    });
                });

                ui.separator();
                ui.horizontal(|ui| {
                    if running {
                        if ui.add(egui::Button::new("Cancel")).clicked() {
                            self.shared.cancel.store(true, Ordering::Relaxed);
                        }
                    } else if ui.add(egui::Button::new(egui::RichText::new("Start").strong().size(18.0))).clicked() {
                        self.start();
                    }
                });

                let st = self.shared.state.lock().unwrap();
                if running {
                    ui.label(format!("{}  {}", st.file, st.status));
                    ui.add(egui::ProgressBar::new(st.frac as f32).show_percentage().animate(true));
                }
                if let Some(msg) = &st.finished {
                    ui.label(egui::RichText::new(msg).strong());
                }
                if let (false, Some(dir)) = (running, st.out_dir.clone()) {
                    if ui.button("Open output folder").clicked() {
                        open_folder(&dir);
                    }
                }
                if !st.log.is_empty() {
                    ui.collapsing("Log", |ui| {
                        egui::ScrollArea::vertical().max_height(160.0).stick_to_bottom(true).show(ui, |ui| {
                            for l in &st.log {
                                ui.monospace(l);
                            }
                        });
                    });
                }
            });
        });
    }
}

pub fn run() -> i32 {
    #[cfg(windows)]
    {
        // Double-clicked: drop the black console window.
        extern "system" {
            fn FreeConsole() -> i32;
        }
        unsafe {
            FreeConsole();
        }
    }
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([560.0, 640.0]).with_min_inner_size([460.0, 480.0]),
        ..Default::default()
    };
    match eframe::run_native("Video Compressor", options, Box::new(|cc| Ok(Box::new(App::new(cc))))) {
        Ok(()) => 0,
        Err(e) => {
            eprintln!("could not open the window: {e}");
            1
        }
    }
}
