use eframe::egui::{self, Color32, RichText, ScrollArea, Vec2};
use image::{DynamicImage, ImageFormat};
use rayon::prelude::*;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([780.0, 620.0])
            .with_min_inner_size([600.0, 400.0])
            .with_drag_and_drop(true)
            .with_title("Image Converter"),
        ..Default::default()
    };

    eframe::run_native(
        "Image Converter",
        options,
        Box::new(|_cc| Ok(Box::new(ImageConverterApp::default()))),
    )
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum TargetFormat {
    Png,
    Jpeg,
    WebP,
    Tiff,
    Bmp,
    Gif,
}

impl TargetFormat {
    fn extension(self) -> &'static str {
        match self {
            TargetFormat::Png => "png",
            TargetFormat::Jpeg => "jpg",
            TargetFormat::WebP => "webp",
            TargetFormat::Tiff => "tiff",
            TargetFormat::Bmp => "bmp",
            TargetFormat::Gif => "gif",
        }
    }

    fn image_format(self) -> ImageFormat {
        match self {
            TargetFormat::Png => ImageFormat::Png,
            TargetFormat::Jpeg => ImageFormat::Jpeg,
            TargetFormat::WebP => ImageFormat::WebP,
            TargetFormat::Tiff => ImageFormat::Tiff,
            TargetFormat::Bmp => ImageFormat::Bmp,
            TargetFormat::Gif => ImageFormat::Gif,
        }
    }

    fn label(self) -> &'static str {
        match self {
            TargetFormat::Png => "PNG",
            TargetFormat::Jpeg => "JPEG",
            TargetFormat::WebP => "WebP",
            TargetFormat::Tiff => "TIFF",
            TargetFormat::Bmp => "BMP",
            TargetFormat::Gif => "GIF",
        }
    }

    fn all() -> &'static [TargetFormat] {
        &[
            TargetFormat::Png,
            TargetFormat::Jpeg,
            TargetFormat::WebP,
            TargetFormat::Tiff,
            TargetFormat::Bmp,
            TargetFormat::Gif,
        ]
    }
}

#[derive(Clone)]
struct FileEntry {
    path: PathBuf,
    status: String, // "Pending", "Converting...", "Done", "Error: ..."
}

struct ImageConverterApp {
    files: Vec<FileEntry>,
    target_format: TargetFormat,
    quality: u8, // 1-100 for JPEG / WebP
    output_dir: Option<PathBuf>,
    use_same_folder: bool,
    is_converting: bool,
    status_message: String,
    // Shared progress for background work
    conversion_results: Arc<Mutex<Vec<(usize, String)>>>,
}

impl Default for ImageConverterApp {
    fn default() -> Self {
        Self {
            files: Vec::new(),
            target_format: TargetFormat::Png,
            quality: 85,
            output_dir: None,
            use_same_folder: true,
            is_converting: false,
            status_message: "Drop image files here or use the button below.".to_string(),
            conversion_results: Arc::new(Mutex::new(Vec::new())),
        }
    }
}

impl eframe::App for ImageConverterApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Handle file drops
        ctx.input(|i| {
            if !i.raw.dropped_files.is_empty() {
                for dropped in &i.raw.dropped_files {
                    if let Some(path) = &dropped.path {
                        if is_supported_image(path) {
                            self.files.push(FileEntry {
                                path: path.clone(),
                                status: "Pending".to_string(),
                            });
                        }
                    }
                }
                if !self.files.is_empty() {
                    self.status_message = format!("{} file(s) ready", self.files.len());
                }
            }
        });

        // Check for finished conversion results
        if self.is_converting {
            if let Ok(mut results) = self.conversion_results.lock() {
                if !results.is_empty() {
                    for (idx, status) in results.drain(..) {
                        if let Some(entry) = self.files.get_mut(idx) {
                            entry.status = status;
                        }
                    }
                }
            }

            // Check if all done
            let all_done = self
                .files
                .iter()
                .all(|f| f.status != "Pending" && f.status != "Converting...");
            if all_done {
                self.is_converting = false;
                let success = self.files.iter().filter(|f| f.status == "Done").count();
                let failed = self.files.len() - success;
                self.status_message = format!(
                    "Finished: {} succeeded, {} failed",
                    success, failed
                );
            }
            ctx.request_repaint(); // keep polling while converting
        }

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading(RichText::new("Image Converter").size(22.0));
            ui.label("Drag & drop multiple images, choose format, then convert.");
            ui.add_space(8.0);

            // Settings row
            ui.horizontal(|ui| {
                ui.label("Target format:");
                egui::ComboBox::from_id_salt("format")
                    .selected_text(self.target_format.label())
                    .show_ui(ui, |ui| {
                        for fmt in TargetFormat::all() {
                            ui.selectable_value(&mut self.target_format, *fmt, fmt.label());
                        }
                    });

                ui.add_space(16.0);

                if matches!(self.target_format, TargetFormat::Jpeg | TargetFormat::WebP) {
                    ui.label("Quality:");
                    ui.add(egui::Slider::new(&mut self.quality, 1..=100).text("%"));
                }
            });

            ui.add_space(4.0);

            ui.horizontal(|ui| {
                ui.checkbox(&mut self.use_same_folder, "Save next to original files");

                if !self.use_same_folder {
                    if ui.button("Choose output folder…").clicked() {
                        if let Some(dir) = rfd::FileDialog::new().pick_folder() {
                            self.output_dir = Some(dir);
                        }
                    }
                    if let Some(ref dir) = self.output_dir {
                        ui.label(format!("→ {}", dir.display()));
                    }
                }
            });

            ui.add_space(8.0);

            // Action buttons
            ui.horizontal(|ui| {
                if ui.button("Add files…").clicked() {
                    if let Some(paths) = rfd::FileDialog::new()
                        .add_filter(
                            "Images",
                            &["png", "jpg", "jpeg", "webp", "gif", "bmp", "tiff", "tif"],
                        )
                        .pick_files()
                    {
                        for path in paths {
                            if is_supported_image(&path) {
                                self.files.push(FileEntry {
                                    path,
                                    status: "Pending".to_string(),
                                });
                            }
                        }
                        self.status_message = format!("{} file(s) ready", self.files.len());
                    }
                }

                if ui.button("Clear list").clicked() {
                    self.files.clear();
                    self.status_message = "List cleared.".to_string();
                }

                ui.add_space(12.0);

                let can_convert = !self.files.is_empty()
                    && !self.is_converting
                    && (self.use_same_folder || self.output_dir.is_some());

                if ui
                    .add_enabled(can_convert, egui::Button::new(RichText::new("Convert").strong()))
                    .clicked()
                {
                    self.start_conversion(ctx);
                }
            });

            ui.add_space(6.0);
            ui.separator();

            // Status
            ui.label(RichText::new(&self.status_message).color(Color32::from_rgb(180, 180, 180)));

            ui.add_space(4.0);

            // File list
            ScrollArea::vertical()
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    if self.files.is_empty() {
                        ui.vertical_centered(|ui| {
                            ui.add_space(40.0);
                            ui.label(
                                RichText::new("Drop image files here")
                                    .size(18.0)
                                    .color(Color32::from_rgb(120, 120, 120)),
                            );
                            ui.label(
                                RichText::new("PNG • JPG • WebP • GIF • BMP • TIFF")
                                    .color(Color32::from_rgb(100, 100, 100)),
                            );
                        });
                    } else {
                        egui::Grid::new("file_grid")
                            .num_columns(3)
                            .spacing([12.0, 4.0])
                            .striped(true)
                            .show(ui, |ui| {
                                ui.label(RichText::new("File").strong());
                                ui.label(RichText::new("Status").strong());
                                ui.label(""); // actions
                                ui.end_row();

                                let mut to_remove = None;
                                for (i, entry) in self.files.iter().enumerate() {
                                    let name = entry
                                        .path
                                        .file_name()
                                        .map(|s| s.to_string_lossy().to_string())
                                        .unwrap_or_else(|| entry.path.display().to_string());

                                    ui.label(&name);

                                    let color = if entry.status == "Done" {
                                        Color32::from_rgb(80, 180, 80)
                                    } else if entry.status.starts_with("Error") {
                                        Color32::from_rgb(220, 80, 80)
                                    } else if entry.status == "Converting..." {
                                        Color32::from_rgb(80, 140, 220)
                                    } else {
                                        Color32::GRAY
                                    };
                                    ui.colored_label(color, &entry.status);

                                    if ui.small_button("✕").clicked() {
                                        to_remove = Some(i);
                                    }
                                    ui.end_row();
                                }

                                if let Some(i) = to_remove {
                                    self.files.remove(i);
                                }
                            });
                    }
                });
        });
    }
}

impl ImageConverterApp {
    fn start_conversion(&mut self, ctx: &egui::Context) {
        self.is_converting = true;
        self.status_message = "Converting…".to_string();

        // Mark all as converting
        for entry in &mut self.files {
            entry.status = "Converting...".to_string();
        }

        let files: Vec<(usize, PathBuf)> = self
            .files
            .iter()
            .enumerate()
            .map(|(i, e)| (i, e.path.clone()))
            .collect();

        let target = self.target_format;
        let quality = self.quality;
        let use_same = self.use_same_folder;
        let out_dir = self.output_dir.clone();
        let results = Arc::clone(&self.conversion_results);
        let ctx = ctx.clone();

        // Run in background thread so UI stays responsive
        std::thread::spawn(move || {
            files.par_iter().for_each(|(idx, path)| {
                let status = convert_one(path, target, quality, use_same, out_dir.as_ref());
                if let Ok(mut guard) = results.lock() {
                    guard.push((*idx, status));
                }
                ctx.request_repaint();
            });
        });
    }
}

fn is_supported_image(path: &Path) -> bool {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_lowercase())
        .unwrap_or_default();

    matches!(
        ext.as_str(),
        "png" | "jpg" | "jpeg" | "webp" | "gif" | "bmp" | "tiff" | "tif"
    )
}

fn convert_one(
    input: &Path,
    target: TargetFormat,
    quality: u8,
    use_same_folder: bool,
    output_dir: Option<&PathBuf>,
) -> String {
    let result = (|| -> anyhow::Result<()> {
        let img = image::open(input)?;

        let out_path = if use_same_folder {
            let mut p = input.to_path_buf();
            p.set_extension(target.extension());
            // Avoid overwriting the original if same format
            if p == input {
                let stem = input
                    .file_stem()
                    .map(|s| s.to_string_lossy().to_string())
                    .unwrap_or_else(|| "converted".into());
                p.set_file_name(format!("{}_converted.{}", stem, target.extension()));
            }
            p
        } else {
            let dir = output_dir.ok_or_else(|| anyhow::anyhow!("No output folder"))?;
            let name = input
                .file_stem()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_else(|| "image".into());
            dir.join(format!("{}.{}", name, target.extension()))
        };

        save_image(&img, &out_path, target, quality)?;
        Ok(())
    })();

    match result {
        Ok(()) => "Done".to_string(),
        Err(e) => format!("Error: {}", e),
    }
}

fn save_image(
    img: &DynamicImage,
    path: &Path,
    target: TargetFormat,
    quality: u8,
) -> anyhow::Result<()> {
    match target {
        TargetFormat::Jpeg => {
            // JPEG has no alpha — convert to RGB
            let rgb = img.to_rgb8();
            let mut file = std::fs::File::create(path)?;
            let mut encoder =
                image::codecs::jpeg::JpegEncoder::new_with_quality(&mut file, quality);
            encoder.encode(
                rgb.as_raw(),
                rgb.width(),
                rgb.height(),
                image::ExtendedColorType::Rgb8,
            )?;
        }
        TargetFormat::WebP => {
            // The image crate currently writes lossless WebP.
            // Still excellent size reduction vs PNG for most photos.
            img.save_with_format(path, ImageFormat::WebP)?;
        }
        _ => {
            img.save_with_format(path, target.image_format())?;
        }
    }
    Ok(())
}
