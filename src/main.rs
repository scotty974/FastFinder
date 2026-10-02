#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod search;

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

use eframe::egui::{self, Color32, FontId, Margin, RichText, Sense, vec2};

type Index = HashMap<String, Vec<PathBuf>>;

const ROOT: &str = r"C:\";
const ROW_HEIGHT: f32 = 42.0;
const ACCENT: Color32 = Color32::from_rgb(0x6e, 0x8b, 0xff);
const DIM: Color32 = Color32::from_rgb(0x8a, 0x90, 0x9c);

fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Recherche")
            .with_inner_size([900.0, 600.0]),
        ..Default::default()
    };
    eframe::run_native(
        "recherche",
        options,
        Box::new(|cc| {
            cc.egui_ctx.set_visuals(egui::Visuals::dark());
            Ok(Box::new(App::new(&cc.egui_ctx)))
        }),
    )
}

struct App {
    index: Option<Index>,
    rx: Option<mpsc::Receiver<Result<(Index, Duration), String>>>,
    started: Instant,
    status: String,
    query: String,
    results: Vec<PathBuf>,
    selected: Option<usize>,
}

impl App {
    fn new(ctx: &egui::Context) -> Self {
        // Indexation dans un thread séparé pour ne pas bloquer l'interface
        let (tx, rx) = mpsc::channel();
        let ctx = ctx.clone();
        thread::spawn(move || {
            let start = Instant::now();
            let result = search::build_index(ROOT)
                .map(|index| (index, start.elapsed()))
                .map_err(|e| e.to_string());
            let _ = tx.send(result);
            ctx.request_repaint();
        });

        Self {
            index: None,
            rx: Some(rx),
            started: Instant::now(),
            status: String::new(),
            query: String::new(),
            results: Vec::new(),
            selected: None,
        }
    }

    fn poll_index(&mut self, ctx: &egui::Context) {
        let Some(rx) = &self.rx else { return };
        match rx.try_recv() {
            Ok(Ok((index, duration))) => {
                self.status = format!("Index prêt en {:.1} s", duration.as_secs_f64());
                self.index = Some(index);
                self.rx = None;
                self.run_search();
            }
            Ok(Err(error)) => {
                self.status = format!("Erreur d'indexation : {error}");
                self.rx = None;
            }
            Err(_) => {
                self.status = format!(
                    "Indexation de {ROOT}… {:.0} s",
                    self.started.elapsed().as_secs_f64()
                );
                ctx.request_repaint_after(Duration::from_millis(250));
            }
        }
    }

    fn run_search(&mut self) {
        self.results.clear();
        self.selected = None;

        let query = self.query.trim();
        let Some(index) = &self.index else { return };
        if query.is_empty() {
            return;
        }

        // Texte simple -> recherche "contient", sinon glob tel quel
        let is_glob = query.chars().any(|c| matches!(c, '*' | '?' | '[' | '{'));
        let pattern = if is_glob { query.to_string() } else { format!("*{query}*") };

        let start = Instant::now();
        match search::search_file(index, &pattern) {
            Ok(mut results) => {
                results.sort();
                self.status = format!(
                    "{} résultat(s) en {:.1} ms",
                    results.len(),
                    start.elapsed().as_secs_f64() * 1000.0
                );
                self.selected = (!results.is_empty()).then_some(0);
                self.results = results;
            }
            Err(error) => self.status = format!("Motif invalide : {error}"),
        }
    }

    fn handle_keys(&mut self, ctx: &egui::Context) {
        if self.results.is_empty() {
            return;
        }
        let (down, up, enter) = ctx.input(|i| {
            (
                i.key_pressed(egui::Key::ArrowDown),
                i.key_pressed(egui::Key::ArrowUp),
                i.key_pressed(egui::Key::Enter),
            )
        });
        let last = self.results.len() - 1;
        let current = self.selected.unwrap_or(0);
        if down {
            self.selected = Some((current + 1).min(last));
        }
        if up {
            self.selected = Some(current.saturating_sub(1));
        }
        if enter && let Some(path) = self.selected.and_then(|i| self.results.get(i)) {
            open(path);
        }
    }
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.poll_index(ui.ctx());
        self.handle_keys(ui.ctx());

        egui::Panel::top("search")
            .frame(egui::Frame::new().inner_margin(Margin::same(16)))
            .show(ui, |ui| {
                let response = ui.add_enabled(
                    self.index.is_some(),
                    egui::TextEdit::singleline(&mut self.query)
                        .font(FontId::proportional(18.0))
                        .desired_width(f32::INFINITY)
                        .margin(Margin::symmetric(12, 10))
                        .hint_text("Nom de fichier ou motif (*.pdf)"),
                );
                if response.changed() {
                    self.run_search();
                }
            });

        egui::Panel::bottom("status")
            .frame(egui::Frame::new().inner_margin(Margin::symmetric(16, 6)))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    if self.rx.is_some() {
                        ui.spinner();
                    }
                    ui.label(RichText::new(&self.status).color(DIM));
                });
            });

        egui::CentralPanel::default_margins().show(ui, |ui| {
            let mut to_open = None;
            egui::ScrollArea::vertical().auto_shrink(false).show_rows(
                ui,
                ROW_HEIGHT,
                self.results.len(),
                |ui, range| {
                    ui.spacing_mut().item_spacing.y = 0.0;
                    for i in range {
                        let response = row(ui, &self.results[i], self.selected == Some(i));
                        if response.clicked() {
                            self.selected = Some(i);
                        }
                        if response.double_clicked() {
                            to_open = Some(i);
                        }
                    }
                },
            );
            if let Some(path) = to_open.and_then(|i| self.results.get(i)) {
                open(path);
            }
        });
    }
}

/// Une ligne : nom du fichier en haut, dossier parent en dessous.
fn row(ui: &mut egui::Ui, path: &Path, selected: bool) -> egui::Response {
    let (rect, response) =
        ui.allocate_exact_size(vec2(ui.available_width(), ROW_HEIGHT), Sense::click());
    let painter = ui.painter_at(rect);

    if selected {
        painter.rect_filled(rect.shrink(1.0), 6.0, ACCENT.gamma_multiply(0.25));
    } else if response.hovered() {
        painter.rect_filled(rect.shrink(1.0), 6.0, ui.visuals().faint_bg_color);
    }

    let name = path.file_name().map(|n| n.to_string_lossy()).unwrap_or_default();
    let parent = path.parent().map(|p| p.display().to_string()).unwrap_or_default();
    let x = rect.left() + 10.0;

    painter.text(
        egui::pos2(x, rect.center().y - 8.0),
        egui::Align2::LEFT_CENTER,
        name,
        FontId::proportional(14.0),
        ui.visuals().strong_text_color(),
    );
    painter.text(
        egui::pos2(x, rect.center().y + 9.0),
        egui::Align2::LEFT_CENTER,
        parent,
        FontId::proportional(11.5),
        DIM,
    );
    response
}

/// Ouvre le fichier avec l'application par défaut de Windows.
fn open(path: &Path) {
    let _ = Command::new("explorer").arg(path).spawn();
}