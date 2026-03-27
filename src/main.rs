mod compressor;
mod decompress;

use eframe::egui;
use rfd::FileDialog;
use std::path::PathBuf;
use std::fs::File;
use std::io::Write;
use rayon::prelude::*;
use std::sync::mpsc;

use rawler::decode_file;
use rawler::rawimage::RawImageData;

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([500.0, 620.0]),
        ..Default::default()
    };

    eframe::run_native(
        "X-Fold Pro | Rawler High Performance",
        options,
        Box::new(|_cc| Box::new(XVaultApp::default())),
    )
}

struct XVaultApp {
    status: String,
    selected_file: Option<PathBuf>,
    selected_folder: Option<PathBuf>,
    folder_files: Vec<PathBuf>,
    tx: mpsc::Sender<String>,
    rx: mpsc::Receiver<String>,
    is_processing: bool,
}

impl Default for XVaultApp {
    fn default() -> Self {
        let (tx, rx) = mpsc::channel();
        Self {
            status: "System Ready".to_string(),
            selected_file: None,
            selected_folder: None,
            folder_files: Vec::new(),
            tx,
            rx,
            is_processing: false,
        }
    }
}

impl XVaultApp {
    fn execute_compression(&mut self, ctx: egui::Context) {
        let files_to_process = if let Some(file) = &self.selected_file {
            vec![file.clone()]
        } else {
            self.folder_files.clone()
        };

        if files_to_process.is_empty() {
            self.status = "Error: No files selected!".to_string();
            return;
        }

        self.is_processing = true;
        self.status = format!("Compressing {} files...", files_to_process.len());
        let tx = self.tx.clone();

        std::thread::spawn(move || {
            files_to_process.par_iter().for_each(|path| {
                match decode_file(path) {
                    Ok(raw_image) => {
                        if let RawImageData::Integer(samples) = raw_image.data {
                            let width = raw_image.width;
                            let height = raw_image.height;

                            // Safety Crop: Ensure dimensions are multiples of 6 for X-Trans 2D math
                            let safe_width = (width / 6) * 6;
                            let safe_height = (height / 6) * 6;

                            // 1. New Compressor call now takes the original path to extract metadata
                            let full_vault_payload = compressor::compress_pixels(
                                path, // <--- New: Passing path for RAF header extraction
                                safe_width,
                                safe_height,
                                &samples,
                            );

                            let mut out_path = path.clone();
                            out_path.set_extension("xvault");

                            // 2. We no longer write manual headers here; the compressor module
                            // handles the full payload (Magic, Biases, Metadata, Pixels)
                            if let Ok(mut out_file) = File::create(out_path) {
                                let _ = out_file.write_all(&full_vault_payload);
                            }
                        }
                    }
                    Err(e) => {
                        eprintln!("Failed to decode {:?}: {}", path, e);
                    }
                }
            });

            let _ = tx.send("Compression Task Finished Successfully.".to_string());
            ctx.request_repaint();
        });
    }

    fn execute_decompression(&mut self, ctx: egui::Context) {
        if let Some(path) = FileDialog::new().add_filter("X-Vault", &["xvault"]).pick_file() {
            self.is_processing = true;
            self.status = "Reconstructing original RAF...".to_string();
            let tx = self.tx.clone();

            std::thread::spawn(move || {
                // 3. Updated Decompress call handles the full RAF reconstruction
                // This function now writes the .RAF file to disk internally.
                decompress::decompress_to_raf(&path);

                let _ = tx.send("✅ Original .RAF restored successfully!".to_string());
                ctx.request_repaint();
            });
        }
    }
}

impl eframe::App for XVaultApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if let Ok(msg) = self.rx.try_recv() {
            self.status = msg;
            self.is_processing = false;
        }

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.vertical_centered(|ui| {
                ui.heading("🚀 X-Fold Pro");
                ui.label(egui::RichText::new("Fuji X-Trans 2D Mathematical Engine").small());
            });
            ui.add_space(20.0);

            ui.group(|ui| {
                ui.set_width(ui.available_width());
                ui.label(egui::RichText::new("📦 COMPRESSION").strong());
                ui.add_space(8.0);

                ui.horizontal(|ui| {
                    if ui.button("📁 Select Fuji RAF").clicked() {
                        if let Some(p) = FileDialog::new().add_filter("Fuji RAF", &["RAF"]).pick_file() {
                            self.selected_file = Some(p);
                            self.selected_folder = None;
                            self.status = "File selected.".to_string();
                        }
                    }

                    if ui.button("🗂 Select Folder").clicked() {
                        if let Some(path) = FileDialog::new().pick_folder() {
                            if let Ok(dir) = std::fs::read_dir(&path) {
                                self.folder_files = dir
                                    .filter_map(|res| res.ok())
                                    .map(|res| res.path())
                                    .filter(|p| p.extension().map_or(false, |ext| ext.to_ascii_lowercase() == "raf"))
                                    .collect();
                                self.selected_folder = Some(path);
                                self.selected_file = None;
                                self.status = format!("Folder loaded: {} files.", self.folder_files.len());
                            }
                        }
                    }
                });

                ui.add_space(10.0);
                if ui.button(egui::RichText::new("⚡ START COMPRESSION").strong().color(egui::Color32::WHITE))
                    .clicked() {
                    self.execute_compression(ctx.clone());
                }
            });

            ui.add_space(20.0);

            ui.group(|ui| {
                ui.set_width(ui.available_width());
                ui.label(egui::RichText::new("🔓 DECOMPRESSION").strong());
                ui.add_space(8.0);
                if ui.button("🔓 SELECT & RESTORE").clicked() {
                    self.execute_decompression(ctx.clone());
                }
            });

            ui.with_layout(egui::Layout::bottom_up(egui::Align::LEFT), |ui| {
                ui.horizontal(|ui| {
                    if self.is_processing { ui.add(egui::Spinner::new()); }
                    ui.label(format!("Status: {}", self.status));
                });
            });
        });
    }
}