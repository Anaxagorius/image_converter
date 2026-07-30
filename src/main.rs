use eframe::egui::{self, Align, Color32, Layout, ProgressBar, RichText, ScrollArea};
use image::{DynamicImage, ImageFormat};
use rayon::prelude::*;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

const APP_VERSION: &str = env!("CARGO_PKG_VERSION");
const APP_NAME: &str = "Image Converter";

fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([920.0, 700.0])
            .with_min_inner_size([660.0, 500.0])
            .with_drag_and_drop(true)
            .with_title(format!("{APP_NAME}  v{APP_VERSION}")),
        ..Default::default()
    };

    eframe::run_native(
        APP_NAME,
        options,
        Box::new(|cc| {
            cc.egui_ctx.set_visuals(app_visuals());
            Ok(Box::new(ImageConverterApp::default()))
        }),
    )
}

fn app_visuals() -> egui::Visuals {
    let mut v = egui::Visuals::dark();
    // Sky-blue accent
    let accent = Color32::from_rgb(56, 189, 248);
    v.hyperlink_color = accent;
    v.selection.bg_fill = Color32::from_rgba_premultiplied(56, 189, 248, 60);
    v
}

// ─── Output format ────────────────────────────────────────────────────────────

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
            Self::Png => "png",
            Self::Jpeg => "jpg",
            Self::WebP => "webp",
            Self::Tiff => "tiff",
            Self::Bmp => "bmp",
            Self::Gif => "gif",
        }
    }

    fn image_format(self) -> ImageFormat {
        match self {
            Self::Png => ImageFormat::Png,
            Self::Jpeg => ImageFormat::Jpeg,
            Self::WebP => ImageFormat::WebP,
            Self::Tiff => ImageFormat::Tiff,
            Self::Bmp => ImageFormat::Bmp,
            Self::Gif => ImageFormat::Gif,
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::Png => "PNG",
            Self::Jpeg => "JPEG",
            Self::WebP => "WebP",
            Self::Tiff => "TIFF",
            Self::Bmp => "BMP",
            Self::Gif => "GIF",
        }
    }

    fn description(self) -> &'static str {
        match self {
            Self::Png => "Lossless · supports transparency",
            Self::Jpeg => "Lossy · great for photos · no alpha",
            Self::WebP => "Modern · excellent compression · supports alpha",
            Self::Tiff => "High quality · large files · wide compatibility",
            Self::Bmp => "Uncompressed bitmap · very large files",
            Self::Gif => "256-colour limit · supports animation",
        }
    }

    fn uses_quality(self) -> bool {
        matches!(self, Self::Jpeg | Self::WebP)
    }

    fn all() -> &'static [Self] {
        &[
            Self::Png,
            Self::Jpeg,
            Self::WebP,
            Self::Tiff,
            Self::Bmp,
            Self::Gif,
        ]
    }
}

// ─── Per-file status ──────────────────────────────────────────────────────────

#[derive(Clone, PartialEq, Eq)]
enum ConversionStatus {
    Pending,
    Converting,
    Done,
    Error(String),
}

impl ConversionStatus {
    fn icon(&self) -> &'static str {
        match self {
            Self::Pending => "⏳",
            Self::Converting => "🔄",
            Self::Done => "✅",
            Self::Error(_) => "❌",
        }
    }

    fn color(&self) -> Color32 {
        match self {
            Self::Pending => Color32::from_rgb(160, 160, 165),
            Self::Converting => Color32::from_rgb(80, 160, 240),
            Self::Done => Color32::from_rgb(72, 199, 116),
            Self::Error(_) => Color32::from_rgb(230, 90, 90),
        }
    }

    fn short_text(&self) -> String {
        match self {
            Self::Pending => "Pending".into(),
            Self::Converting => "Converting…".into(),
            Self::Done => "Done".into(),
            Self::Error(e) => {
                if e.len() > 55 {
                    format!("{}…", &e[..52])
                } else {
                    e.clone()
                }
            }
        }
    }

    fn is_terminal(&self) -> bool {
        matches!(self, Self::Done | Self::Error(_))
    }

    fn is_retriable(&self) -> bool {
        matches!(self, Self::Pending | Self::Error(_))
    }
}

// ─── File entry ───────────────────────────────────────────────────────────────

#[derive(Clone)]
struct FileEntry {
    path: PathBuf,
    size_bytes: u64,
    status: ConversionStatus,
}

impl FileEntry {
    fn new(path: PathBuf) -> Self {
        let size_bytes = std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
        Self {
            path,
            size_bytes,
            status: ConversionStatus::Pending,
        }
    }

    fn display_name(&self) -> String {
        self.path
            .file_name()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| self.path.display().to_string())
    }

    fn display_size(&self) -> String {
        format_bytes(self.size_bytes)
    }
}

fn format_bytes(b: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = 1024 * KB;
    const GB: u64 = 1024 * MB;
    if b < KB {
        format!("{b} B")
    } else if b < MB {
        format!("{:.1} KB", b as f64 / KB as f64)
    } else if b < GB {
        format!("{:.1} MB", b as f64 / MB as f64)
    } else {
        format!("{:.2} GB", b as f64 / GB as f64)
    }
}

// ─── Application ──────────────────────────────────────────────────────────────

struct ImageConverterApp {
    files: Vec<FileEntry>,
    target_format: TargetFormat,
    quality: u8,
    output_dir: Option<PathBuf>,
    use_same_folder: bool,
    is_converting: bool,
    status_message: String,
    /// Results pushed by the background thread: (file index, new status).
    conversion_results: Arc<Mutex<Vec<(usize, ConversionStatus)>>>,
    /// Folder where the last batch was written — shown in the "Open folder" button.
    last_output_dir: Option<PathBuf>,
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
            status_message: String::new(),
            conversion_results: Arc::new(Mutex::new(Vec::new())),
            last_output_dir: None,
        }
    }
}

// ── Helper queries ─────────────────────────────────────────────────────────────

impl ImageConverterApp {
    fn done_count(&self) -> usize {
        self.files
            .iter()
            .filter(|f| f.status == ConversionStatus::Done)
            .count()
    }

    fn error_count(&self) -> usize {
        self.files
            .iter()
            .filter(|f| matches!(f.status, ConversionStatus::Error(_)))
            .count()
    }

    fn converting_count(&self) -> usize {
        self.files
            .iter()
            .filter(|f| f.status == ConversionStatus::Converting)
            .count()
    }

    fn retriable_count(&self) -> usize {
        self.files
            .iter()
            .filter(|f| f.status.is_retriable())
            .count()
    }

    fn conversion_progress(&self) -> f32 {
        if self.files.is_empty() {
            return 0.0;
        }
        self.files.iter().filter(|f| f.status.is_terminal()).count() as f32
            / self.files.len() as f32
    }

    fn can_convert(&self) -> bool {
        !self.is_converting
            && self.retriable_count() > 0
            && (self.use_same_folder || self.output_dir.is_some())
    }
}

// ── File management ───────────────────────────────────────────────────────────

impl ImageConverterApp {
    /// Add paths, skipping duplicates and unsupported formats.
    fn add_paths(&mut self, paths: impl IntoIterator<Item = PathBuf>) {
        let mut added = 0usize;
        let mut skipped = 0usize;

        for path in paths {
            if !is_supported_image(&path) {
                skipped += 1;
                continue;
            }
            if self.files.iter().any(|f| f.path == path) {
                skipped += 1;
                continue;
            }
            self.files.push(FileEntry::new(path));
            added += 1;
        }

        let total = self.files.len();
        self.status_message = if added == 0 && skipped > 0 {
            format!("No new files added — {skipped} already in list or unsupported.")
        } else if skipped > 0 {
            format!("{total} file(s) ready  •  {skipped} skipped (duplicate / unsupported)")
        } else {
            format!("{total} file(s) ready to convert")
        };
    }

    fn start_conversion(&mut self, ctx: &egui::Context) {
        self.is_converting = true;
        self.status_message = "Converting… please wait.".to_string();

        // Determine the output folder to surface after completion.
        self.last_output_dir = if self.use_same_folder {
            self.files
                .first()
                .and_then(|e| e.path.parent().map(PathBuf::from))
        } else {
            self.output_dir.clone()
        };

        // Clear stale results from any previous run.
        if let Ok(mut r) = self.conversion_results.lock() {
            r.clear();
        }

        // Mark retriable files as "Converting".
        for entry in &mut self.files {
            if entry.status.is_retriable() {
                entry.status = ConversionStatus::Converting;
            }
        }

        let files: Vec<(usize, PathBuf)> = self
            .files
            .iter()
            .enumerate()
            .filter(|(_, e)| e.status == ConversionStatus::Converting)
            .map(|(i, e)| (i, e.path.clone()))
            .collect();

        let target = self.target_format;
        let quality = self.quality;
        let use_same = self.use_same_folder;
        let out_dir = self.output_dir.clone();
        let results = Arc::clone(&self.conversion_results);
        let ctx = ctx.clone();

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

// ── eframe::App ───────────────────────────────────────────────────────────────

impl eframe::App for ImageConverterApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();

        // Handle dropped files.
        let dropped: Vec<PathBuf> = ctx.input(|i| {
            i.raw
                .dropped_files
                .iter()
                .filter_map(|d| d.path.clone())
                .collect()
        });
        if !dropped.is_empty() {
            self.add_paths(dropped);
        }

        // Enter key triggers Convert when possible.
        if ctx.input(|i| i.key_pressed(egui::Key::Enter)) && self.can_convert() {
            self.start_conversion(&ctx);
        }

        // Poll results from the background thread.
        if self.is_converting {
            if let Ok(mut results) = self.conversion_results.lock() {
                for (idx, status) in results.drain(..) {
                    if let Some(entry) = self.files.get_mut(idx) {
                        entry.status = status;
                    }
                }
            }

            if self.converting_count() == 0 {
                self.is_converting = false;
                let success = self.done_count();
                let failed = self.error_count();
                self.status_message = match (success, failed) {
                    (s, 0) => format!("✅  All {s} file(s) converted successfully!"),
                    (0, f) => format!("❌  All {f} conversion(s) failed."),
                    (s, f) => format!("⚠️  {s} succeeded, {f} failed.  Click Convert to retry failures."),
                };
            }

            ctx.request_repaint();
        }

        self.ui_header(ui);
        ui.add_space(8.0);
        self.ui_settings(ui);
        ui.add_space(8.0);
        self.ui_file_section(ui, &ctx);
        self.ui_status_bar(ui);
    }
}

// ── UI sections ───────────────────────────────────────────────────────────────

impl ImageConverterApp {
    fn ui_header(&self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.heading(RichText::new("🖼  Image Converter").size(24.0).strong());
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.weak(format!("v{APP_VERSION}"));
            });
        });
        ui.label(
            RichText::new(
                "Batch-convert images between PNG, JPEG, WebP, TIFF, BMP and GIF. \
                 Drag & drop files or use the buttons below.",
            )
            .color(Color32::from_rgb(145, 145, 160))
            .small(),
        );
    }

    fn ui_settings(&mut self, ui: &mut egui::Ui) {
        ui.group(|ui| {
            ui.label(RichText::new("⚙  Settings").strong());
            ui.add_space(4.0);

            // Format + quality
            ui.horizontal(|ui| {
                ui.label("Output format:");

                egui::ComboBox::from_id_salt("format_combo")
                    .selected_text(self.target_format.label())
                    .width(80.0)
                    .show_ui(ui, |ui| {
                        for fmt in TargetFormat::all() {
                            ui.selectable_value(&mut self.target_format, *fmt, fmt.label())
                                .on_hover_text(fmt.description());
                        }
                    });

                ui.label(
                    RichText::new(self.target_format.description())
                        .color(Color32::from_rgb(130, 130, 150))
                        .small(),
                );

                if self.target_format.uses_quality() {
                    ui.add_space(16.0);
                    ui.label("Quality:");
                    ui.add(egui::Slider::new(&mut self.quality, 1..=100).text("%"))
                        .on_hover_text(
                            "Higher value = better quality and larger file size.\n\
                             85 is a good default for most use-cases.",
                        );
                }
            });

            ui.add_space(4.0);

            // Output destination
            ui.horizontal(|ui| {
                ui.checkbox(&mut self.use_same_folder, "Save next to original files")
                    .on_hover_text(
                        "Each converted file is placed in the same folder as its source image.",
                    );

                if !self.use_same_folder {
                    if ui
                        .button("📂  Choose folder…")
                        .on_hover_text("Pick the folder where converted files will be saved")
                        .clicked()
                    {
                        if let Some(dir) = rfd::FileDialog::new().pick_folder() {
                            self.output_dir = Some(dir);
                        }
                    }

                    match &self.output_dir {
                        Some(dir) => {
                            ui.colored_label(
                                Color32::from_rgb(56, 189, 248),
                                format!("→  {}", dir.display()),
                            );
                        }
                        None => {
                            ui.colored_label(
                                Color32::from_rgb(230, 150, 50),
                                "⚠  No output folder selected",
                            );
                        }
                    }
                }
            });
        });
    }

    fn ui_file_section(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        let total = self.files.len();
        let done = self.done_count();
        let errors = self.error_count();

        // ── Toolbar ──────────────────────────────────────────────────────────
        ui.horizontal(|ui| {
            ui.label(
                RichText::new(if total == 0 {
                    "Files".to_string()
                } else {
                    format!("Files  ({total})")
                })
                .strong(),
            );

            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                // Convert / Retry button
                let can = self.can_convert();
                let btn_label = if self.is_converting {
                    "⏳  Converting…".to_string()
                } else if done + errors == total && errors > 0 && done < total {
                    format!("🔁  Retry  ({errors} failed)")
                } else {
                    "▶  Convert".to_string()
                };

                if ui
                    .add_enabled(
                        can,
                        egui::Button::new(RichText::new(&btn_label).strong())
                            .min_size(egui::Vec2::new(130.0, 0.0)),
                    )
                    .on_hover_text("Start converting all pending files  [Enter]")
                    .clicked()
                {
                    self.start_conversion(ctx);
                }

                // Quick-filter removals (only when idle)
                if !self.is_converting {
                    if errors > 0
                        && ui
                            .button(format!("Remove {errors} error(s)"))
                            .on_hover_text("Remove files that failed to convert")
                            .clicked()
                    {
                        self.files
                            .retain(|f| !matches!(f.status, ConversionStatus::Error(_)));
                    }

                    if done > 0
                        && ui
                            .button(format!("Remove {done} done"))
                            .on_hover_text("Remove successfully converted files from the list")
                            .clicked()
                    {
                        self.files.retain(|f| f.status != ConversionStatus::Done);
                    }

                    if total > 0
                        && ui
                            .button("Clear all")
                            .on_hover_text("Remove all files from the list")
                            .clicked()
                    {
                        self.files.clear();
                        self.last_output_dir = None;
                        self.status_message = "List cleared.".to_string();
                    }
                }

                // Add files button
                if ui
                    .button("➕  Add files…")
                    .on_hover_text(
                        "Open a file picker to add more images.\n\
                         You can also drag & drop files directly onto the window.",
                    )
                    .clicked()
                {
                    if let Some(paths) = rfd::FileDialog::new()
                        .add_filter(
                            "Images",
                            &["png", "jpg", "jpeg", "webp", "gif", "bmp", "tiff", "tif"],
                        )
                        .pick_files()
                    {
                        self.add_paths(paths);
                    }
                }
            });
        });

        ui.separator();

        // ── Progress bar ──────────────────────────────────────────────────────
        if self.is_converting || (total > 0 && done + errors == total) {
            let progress = self.conversion_progress();
            ui.add(
                ProgressBar::new(progress)
                    .show_percentage()
                    .animate(self.is_converting),
            );
            if self.is_converting {
                ui.label(
                    RichText::new(format!(
                        "{}/{total} done  •  {done} succeeded  •  {errors} failed",
                        done + errors
                    ))
                    .small()
                    .color(Color32::from_rgb(130, 130, 155)),
                );
            }
            ui.add_space(4.0);
        }

        // ── File list ─────────────────────────────────────────────────────────
        let available_height = ui.available_height() - 46.0;
        ScrollArea::vertical()
            .auto_shrink([false, false])
            .max_height(available_height)
            .show(ui, |ui| {
                if self.files.is_empty() {
                    ui.vertical_centered(|ui| {
                        ui.add_space(55.0);
                        ui.label(
                            RichText::new("📂  Drop image files here")
                                .size(20.0)
                                .color(Color32::from_rgb(130, 130, 150)),
                        );
                        ui.add_space(6.0);
                        ui.label(
                            RichText::new("PNG  ·  JPEG  ·  WebP  ·  GIF  ·  BMP  ·  TIFF")
                                .color(Color32::from_rgb(95, 95, 115)),
                        );
                        ui.add_space(12.0);
                        if ui.button("➕  Add files…").clicked() {
                            if let Some(paths) = rfd::FileDialog::new()
                                .add_filter(
                                    "Images",
                                    &[
                                        "png", "jpg", "jpeg", "webp", "gif", "bmp", "tiff", "tif",
                                    ],
                                )
                                .pick_files()
                            {
                                self.add_paths(paths);
                            }
                        }
                    });
                } else {
                    egui::Grid::new("file_grid")
                        .num_columns(4)
                        .spacing([12.0, 4.0])
                        .striped(true)
                        .show(ui, |ui| {
                            ui.label(RichText::new("Filename").strong());
                            ui.label(RichText::new("Size").strong());
                            ui.label(RichText::new("Status").strong());
                            ui.label(""); // remove-button column
                            ui.end_row();

                            let mut to_remove: Option<usize> = None;

                            for (i, entry) in self.files.iter().enumerate() {
                                let name = entry.display_name();
                                let display_name = if name.len() > 50 {
                                    format!("{}…", &name[..47])
                                } else {
                                    name.clone()
                                };

                                ui.label(&display_name)
                                    .on_hover_text(entry.path.display().to_string());

                                ui.label(
                                    RichText::new(entry.display_size())
                                        .color(Color32::from_rgb(130, 130, 155)),
                                );

                                let status_label = ui.colored_label(
                                    entry.status.color(),
                                    format!("{}  {}", entry.status.icon(), entry.status.short_text()),
                                );
                                // Show full error message on hover.
                                if let ConversionStatus::Error(e) = &entry.status {
                                    status_label.on_hover_text(e.as_str());
                                }

                                let remove = ui
                                    .add_enabled(
                                        !self.is_converting,
                                        egui::Button::new("✕").small(),
                                    )
                                    .on_hover_text("Remove from list");
                                if remove.clicked() {
                                    to_remove = Some(i);
                                }

                                ui.end_row();
                            }

                            if let Some(i) = to_remove {
                                self.files.remove(i);
                                if self.files.is_empty() {
                                    self.status_message = "List cleared.".to_string();
                                    self.last_output_dir = None;
                                }
                            }
                        });
                }
            });
    }

    fn ui_status_bar(&mut self, ui: &mut egui::Ui) {
        ui.separator();
        ui.horizontal(|ui| {
            let msg = if self.status_message.is_empty() {
                "Drop images here or click \"➕  Add files…\" to get started."
            } else {
                &self.status_message
            };
            ui.label(RichText::new(msg).color(Color32::from_rgb(160, 160, 175)));

            // "Open output folder" appears after a batch finishes.
            if !self.is_converting {
                if let Some(dir) = self.last_output_dir.clone() {
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if ui
                            .button("📂  Open folder")
                            .on_hover_text(format!("Reveal in file manager:\n{}", dir.display()))
                            .clicked()
                        {
                            open_folder(&dir);
                        }
                    });
                }
            }
        });
    }
}

// ─── Helpers ──────────────────────────────────────────────────────────────────

/// Open `path` in the platform's default file manager.
fn open_folder(path: &Path) {
    #[cfg(target_os = "windows")]
    let _ = std::process::Command::new("explorer").arg(path).spawn();
    #[cfg(target_os = "macos")]
    let _ = std::process::Command::new("open").arg(path).spawn();
    #[cfg(target_os = "linux")]
    let _ = std::process::Command::new("xdg-open").arg(path).spawn();
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
) -> ConversionStatus {
    let result = (|| -> anyhow::Result<()> {
        let img = image::open(input)?;

        let out_path = if use_same_folder {
            let mut p = input.to_path_buf();
            p.set_extension(target.extension());
            // Avoid overwriting the source when the format matches.
            if p == input {
                let stem = input
                    .file_stem()
                    .map(|s| s.to_string_lossy().into_owned())
                    .unwrap_or_else(|| "converted".into());
                p.set_file_name(format!("{stem}_converted.{}", target.extension()));
            }
            p
        } else {
            let dir = output_dir.ok_or_else(|| anyhow::anyhow!("No output folder selected"))?;
            let stem = input
                .file_stem()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_else(|| "image".into());
            dir.join(format!("{stem}.{}", target.extension()))
        };

        save_image(&img, &out_path, target, quality)?;
        Ok(())
    })();

    match result {
        Ok(()) => ConversionStatus::Done,
        Err(e) => ConversionStatus::Error(e.to_string()),
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
            // JPEG has no alpha channel — convert to RGB first.
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
            // The `image` crate writes lossless WebP; still offers excellent
            // compression versus PNG for most photos.
            img.save_with_format(path, ImageFormat::WebP)?;
        }
        _ => {
            img.save_with_format(path, target.image_format())?;
        }
    }
    Ok(())
}
