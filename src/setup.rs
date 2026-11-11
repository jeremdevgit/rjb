// SPDX-License-Identifier: GPL-2.0-or-later — Copyright (C) 2026 jeremdevgit — voir LICENSE
//! Installateur et outil de maintenance (réparer / désinstaller), dans le
//! même style « liquid glass » que l'application. Ils font partie de rJb.exe :
//! le même fichier sert d'installateur (nommé rJb-Setup.exe) et de programme
//! de désinstallation (uninstall.exe dans le dossier d'installation).

use crate::install::{self, Options};
use crate::langue::{tr, trd, trf};
use crate::sys;
use crate::ui::{bold, card, chip, glass, glow, label_dim, section, toggle, ACCENT, DIM, GREEN, TEXT};
use eframe::egui::{self, pos2, vec2, Align, Color32, CornerRadius, Frame, Layout, Margin, RichText, TextureHandle, Ui};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

/// Une désinstallation a réussi : le programme s'efface en quittant.
static UNINSTALLED: AtomicBool = AtomicBool::new(false);

// ── points d'entrée ─────────────────────────────────────────────────────

/// Relance en administrateur avec les mêmes arguments ; renvoie le code de
/// sortie de l'instance élevée, ou None si l'utilisateur a refusé.
fn elevate_self(args: &[String], wait: bool) -> Option<u32> {
    let exe = std::env::current_exe().ok()?;
    let params: Vec<String> = args.iter().skip(1).map(|a| format!("\"{}\"", a.replace('"', ""))).collect();
    sys::run_elevated(&exe, &params.join(" "), wait)
}

fn require_admin(args: &[String], what: &str) -> bool {
    if crate::is_elevated() {
        return true;
    }
    if elevate_self(args, false).is_none() {
        sys::message("rJb", &trf("Les droits administrateur sont nécessaires pour {action}.", &[("action", tr(what))]), true);
    }
    false
}

pub fn run_setup(args: &[String]) {
    let quiet = args.iter().any(|a| a == "--quiet" || a.eq_ignore_ascii_case("/S"));
    if quiet {
        if !crate::is_elevated() {
            let code = elevate_self(args, true).unwrap_or(1);
            std::process::exit(code as i32);
        }
        let mut o = Options::current();
        if let Some(i) = args.iter().position(|a| a == "--dir") {
            if let Some(d) = args.get(i + 1) {
                match install::normalize_dir(d) {
                    Ok(d) => o.dir = d,
                    Err(_) => std::process::exit(2),
                }
            }
        }
        o.pawnio = args.iter().any(|a| a == "--pawnio") && o.pawnio;
        std::process::exit(if install::install(&o, &|_| {}).is_ok() { 0 } else { 1 });
    }
    if !require_admin(args, "installer rJb") {
        return;
    }
    window(tr("Installation de rJb"), Mode::Setup);
}

pub fn run_maintenance() {
    let mut args: Vec<String> = std::env::args().collect();
    if !args.iter().any(|a| a == "--maintenance") {
        args.push("--maintenance".into());
    }
    if !require_admin(&args, "modifier ou désinstaller rJb") {
        return;
    }
    window(tr("Désinstaller ou réparer rJb"), Mode::Maintenance);
}

/// `--uninstall [--all] [--quiet]` : désinstallation sans fenêtre.
pub fn quiet_uninstall(everything: bool) -> i32 {
    let args: Vec<String> = std::env::args().collect();
    if !crate::is_elevated() {
        return elevate_self(&args, true).unwrap_or(1) as i32;
    }
    let r = install::uninstall(everything, &|_| {});
    install::finish_uninstall();
    if r.is_ok() {
        0
    } else {
        1
    }
}

// ── fenêtre ─────────────────────────────────────────────────────────────

#[derive(Clone, Copy, PartialEq)]
enum Mode {
    Setup,
    Maintenance,
}

#[derive(Clone, Copy, PartialEq)]
enum Task {
    Install,
    Repair,
    UninstallApp,
    UninstallAll,
}

#[derive(Default)]
struct Progress {
    step: String,
    result: Option<Result<Vec<String>, String>>,
}

enum Page {
    Choose,
    Working(Task),
    Done(Task),
}

struct SetupApp {
    mode: Mode,
    page: Page,
    opts: Options,
    dir_text: String,
    dir_error: Option<String>,
    existing: bool,
    existing_version: Option<String>,
    pawn_installed: bool,
    task: Task,
    progress: Arc<Mutex<Progress>>,
    launch: bool,
    logo: TextureHandle,
    glass: bool,
}

fn window(title: &str, mode: Mode) {
    let icon = crate::icon::render_icon(64);
    let opts = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title(title)
            .with_inner_size(sys::fit_window(720.0, 760.0))
            .with_min_inner_size([560.0, 420.0])
            .with_resizable(true)
            .with_maximize_button(false)
            .with_transparent(true)
            .with_icon(egui::IconData { rgba: icon, width: 64, height: 64 }),
        centered: true,
        ..Default::default()
    };
    let _ = eframe::run_native(
        title,
        opts,
        Box::new(move |cc| {
            let glass = crate::ui::apply_backdrop(cc);
            crate::ui::setup_style(&cc.egui_ctx);
            let rgba = crate::icon::render_icon(96);
            let logo = cc.egui_ctx.load_texture(
                "logo",
                egui::ColorImage::from_rgba_unmultiplied([96, 96], &rgba),
                egui::TextureOptions::LINEAR,
            );
            let opts = Options::current();
            Ok(Box::new(SetupApp {
                mode,
                page: Page::Choose,
                dir_text: opts.dir.to_string_lossy().into_owned(),
                dir_error: None,
                existing: install::installed_dir().is_some(),
                existing_version: install::installed_version(),
                pawn_installed: crate::drivers::pawnio::installed(),
                opts,
                task: if mode == Mode::Setup { Task::Install } else { Task::Repair },
                progress: Arc::new(Mutex::new(Progress::default())),
                launch: true,
                logo,
                glass,
            }))
        }),
    );
    if UNINSTALLED.load(Ordering::Relaxed) {
        install::finish_uninstall();
    }
}

/// Bouton dessiné à la main : les styles d'egui le rendaient terne sur le
/// fond translucide. `main` = action principale (violet vif), sinon verre clair.
fn button(ui: &mut Ui, text: &str, main: bool, enabled: bool) -> bool {
    let font = bold(if main { 15.5 } else { 14.5 });
    let galley = ui.painter().layout_no_wrap(tr(text).to_string(), font, Color32::WHITE);
    let size = vec2((galley.size().x + 48.0).max(if main { 170.0 } else { 110.0 }), 42.0);
    let sense = if enabled { egui::Sense::click() } else { egui::Sense::hover() };
    let (rect, resp) = ui.allocate_exact_size(size, sense);
    let hovered = enabled && resp.hovered();
    let pressed = enabled && resp.is_pointer_button_down_on();
    let p = ui.painter();
    let cr = CornerRadius::same(12);
    if main {
        let base = if !enabled {
            Color32::from_rgb(80, 74, 110)
        } else if pressed {
            Color32::from_rgb(110, 78, 240)
        } else if hovered {
            Color32::from_rgb(156, 128, 255)
        } else {
            Color32::from_rgb(138, 106, 255)
        };
        if enabled {
            glow(p, rect.center(), rect.width() * 0.7, Color32::from_rgba_unmultiplied(124, 92, 255, if hovered { 70 } else { 45 }));
        }
        p.rect_filled(rect, cr, base);
        p.rect_stroke(rect, cr, egui::Stroke::new(1.0_f32, Color32::from_rgba_unmultiplied(255, 255, 255, 90)), egui::StrokeKind::Inside);
    } else {
        p.rect_filled(rect, cr, Color32::from_rgba_unmultiplied(255, 255, 255, if hovered { 46 } else { 30 }));
        p.rect_stroke(rect, cr, egui::Stroke::new(1.0_f32, Color32::from_rgba_unmultiplied(255, 255, 255, 70)), egui::StrokeKind::Inside);
    }
    let col = if enabled { Color32::WHITE } else { Color32::from_gray(170) };
    p.galley(rect.center() - galley.size() / 2.0, galley, col);
    resp.on_hover_cursor(if enabled { egui::CursorIcon::PointingHand } else { egui::CursorIcon::Default }).clicked()
}

fn primary(ui: &mut Ui, text: &str, enabled: bool) -> bool {
    button(ui, text, true, enabled)
}

fn secondary(ui: &mut Ui, text: &str) -> bool {
    button(ui, text, false, true)
}

/// Carte d'option sélectionnable (réparer / désinstaller…).
fn choice(ui: &mut Ui, selected: bool, title: &str, sub: &str) -> bool {
    let w = ui.available_width();
    let (rect, resp) = ui.allocate_exact_size(vec2(w, 66.0), egui::Sense::click());
    let p = ui.painter();
    let cr = CornerRadius::same(14);
    if selected {
        p.rect_filled(rect, cr, Color32::from_rgba_unmultiplied(124, 92, 255, 80));
        p.rect_stroke(rect, cr, egui::Stroke::new(1.0_f32, Color32::from_rgba_unmultiplied(170, 150, 255, 140)), egui::StrokeKind::Inside);
    } else {
        p.rect_filled(rect, cr, glass(if resp.hovered() { 22 } else { 12 }));
        p.rect_stroke(rect, cr, egui::Stroke::new(1.0_f32, glass(26)), egui::StrokeKind::Inside);
    }
    let c = pos2(rect.left() + 24.0, rect.center().y);
    p.circle_stroke(c, 8.0, egui::Stroke::new(1.5_f32, if selected { TEXT } else { DIM }));
    if selected {
        p.circle_filled(c, 4.5, TEXT);
    }
    p.text(pos2(rect.left() + 46.0, rect.top() + 11.0), egui::Align2::LEFT_TOP, tr(title), bold(16.0), TEXT);
    p.text(pos2(rect.left() + 46.0, rect.top() + 36.0), egui::Align2::LEFT_TOP, tr(sub), egui::FontId::proportional(13.5), SOFT);
    resp.on_hover_cursor(egui::CursorIcon::PointingHand).clicked()
}

/// Texte secondaire, plus lisible que le gris de l'application sur le verre.
const SOFT: Color32 = Color32::from_rgb(206, 213, 229);

fn note(ui: &mut Ui, t: &str) {
    ui.label(RichText::new(trd(t)).color(SOFT).size(13.5));
}

impl SetupApp {
    fn background(&self, ui: &Ui) {
        let r = ui.ctx().content_rect();
        let p = ui.ctx().layer_painter(egui::LayerId::background());
        // fond plus opaque que l'application : du texte à lire, pas des couleurs à montrer
        let base = if self.glass { Color32::from_rgba_unmultiplied(10, 11, 20, 200) } else { Color32::from_rgb(13, 15, 26) };
        p.rect_filled(r, CornerRadius::ZERO, base);
        glow(&p, pos2(r.left() + r.width() * 0.1, r.top() + r.height() * 0.08), r.width() * 0.45, Color32::from_rgba_unmultiplied(255, 46, 99, 40));
        glow(&p, pos2(r.right() - r.width() * 0.05, r.top() + r.height() * 0.35), r.width() * 0.5, Color32::from_rgba_unmultiplied(0, 170, 255, 34));
        glow(&p, pos2(r.left() + r.width() * 0.4, r.bottom() + r.height() * 0.05), r.width() * 0.5, Color32::from_rgba_unmultiplied(40, 230, 140, 28));
    }

    fn header(&self, ui: &mut Ui) {
        ui.horizontal(|ui| {
            ui.add(egui::Image::new(&self.logo).fit_to_exact_size(vec2(52.0, 52.0)));
            ui.vertical(|ui| {
                let title = match (self.mode, self.existing) {
                    (Mode::Maintenance, _) => "Gérer rJb",
                    (Mode::Setup, true) => "Mettre à jour rJb",
                    (Mode::Setup, false) => "Installer rJb",
                };
                ui.label(RichText::new(tr(title)).font(bold(26.0)));
                label_dim(ui, &format!("Contrôle RGB unifié · version {}", env!("CARGO_PKG_VERSION")));
            });
        });
        ui.add_space(12.0);
    }

    fn start(&mut self, task: Task, ctx: &egui::Context) {
        self.task = task;
        self.page = Page::Working(task);
        let progress = self.progress.clone();
        *progress.lock().unwrap() = Progress::default();
        let opts = self.opts.clone();
        let ctx = ctx.clone();
        std::thread::spawn(move || {
            let step = |s: &str| {
                progress.lock().unwrap().step = s.to_string();
                ctx.request_repaint();
            };
            let r = match task {
                Task::Install => install::install(&opts, &step),
                Task::Repair => install::repair(&step),
                Task::UninstallApp => install::uninstall(false, &step).map(|_| Vec::new()),
                Task::UninstallAll => install::uninstall(true, &step).map(|_| Vec::new()),
            };
            if r.is_ok() && matches!(task, Task::UninstallApp | Task::UninstallAll) {
                UNINSTALLED.store(true, Ordering::Relaxed);
            }
            progress.lock().unwrap().result = Some(r);
            ctx.request_repaint();
        });
    }

    fn setup_page(&mut self, ui: &mut Ui) {
        card(ui, |ui| {
            ui.set_width(ui.available_width());
            section(ui, "Dossier d'installation");
            ui.horizontal(|ui| {
                let r = ui.add(egui::TextEdit::singleline(&mut self.dir_text).desired_width(ui.available_width() - 110.0));
                if r.changed() {
                    self.dir_error = None;
                }
                if ui.button(tr("Parcourir…")).clicked() {
                    if let Some(p) = sys::pick_folder(tr("Choisissez où installer rJb")) {
                        let p = if p.file_name().is_some_and(|n| n.eq_ignore_ascii_case("rJb")) { p } else { p.join("rJb") };
                        self.dir_text = p.to_string_lossy().into_owned();
                        self.dir_error = None;
                    }
                }
            });
            if let Some(e) = &self.dir_error {
                ui.label(RichText::new(trd(e)).color(Color32::from_rgb(255, 120, 120)).size(12.5));
            } else if !install::secure_dir(&PathBuf::from(&self.dir_text)) {
                note(ui, "⚠ Hors de Program Files, rJb démarrera avec Windows sans droits administrateur (RAM et carte mère après un clic).");
            }
            if self.existing {
                let t = match self.existing_version.as_deref() {
                    Some(v) => trf("La version {v} déjà installée sera remplacée.", &[("v", v)]),
                    None => tr("La version déjà installée sera remplacée.").to_string(),
                };
                note(ui, &format!("{t} {}", tr("Vos profils et réglages sont conservés.")));
            }
        });
        ui.add_space(8.0);
        card(ui, |ui| {
            ui.set_width(ui.available_width());
            section(ui, "Raccourcis");
            ui.horizontal(|ui| {
                toggle(ui, &mut self.opts.desktop, "Bureau");
                ui.add_space(18.0);
                toggle(ui, &mut self.opts.start_menu, "Menu Démarrer");
            });
            note(ui, "Barre des tâches : Windows réserve l'épinglage à l'utilisateur. Clic droit sur rJb dans le menu Démarrer › Épingler à la barre des tâches.");
        });
        ui.add_space(8.0);
        card(ui, |ui| {
            ui.set_width(ui.available_width());
            section(ui, "Démarrage et fermeture");
            ui.horizontal(|ui| {
                toggle(ui, &mut self.opts.autostart, "Lancer rJb au démarrage du PC");
            });
            ui.horizontal(|ui| {
                ui.label(tr("Bouton fermer :"));
                if chip(ui, self.opts.close_to_tray, "Réduire dans la zone de notification").clicked() {
                    self.opts.close_to_tray = true;
                }
                if chip(ui, !self.opts.close_to_tray, "Quitter rJb").clicked() {
                    self.opts.close_to_tray = false;
                }
            });
            note(ui, "Modifiable à tout moment dans les Réglages de rJb.");
        });
        ui.add_space(8.0);
        card(ui, |ui| {
            ui.set_width(ui.available_width());
            section(ui, "RAM et carte mère");
            if self.pawn_installed {
                ui.label(RichText::new(tr("✔ Le pilote PawnIO est déjà installé.")).color(GREEN));
            } else {
                ui.horizontal(|ui| {
                    toggle(ui, &mut self.opts.pawnio, "Installer le pilote PawnIO (recommandé, via winget)");
                });
                note(ui, "Pilote signé, libre et partagé avec OpenRGB, FanControl… Sans lui, seuls les périphériques USB sont pilotés.");
            }
        });
    }

    fn setup_actions(&mut self, ui: &mut Ui, ctx: &egui::Context) {
        ui.horizontal(|ui| {
            label_dim(ui, "rJb est un logiciel libre (GPL-2.0-or-later) créé par jeremdevgit.");
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                let label = if self.existing { "Mettre à jour" } else { "Installer" };
                if primary(ui, label, true) {
                    match install::normalize_dir(&self.dir_text) {
                        Ok(d) => {
                            self.opts.dir = d;
                            self.start(Task::Install, ctx);
                        }
                        Err(e) => self.dir_error = Some(e),
                    }
                }
                if secondary(ui, "Annuler") {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                }
            });
        });
    }

    fn maintenance_page(&mut self, ui: &mut Ui) {
        card(ui, |ui| {
            ui.set_width(ui.available_width());
            section(ui, "Que voulez-vous faire ?");
            ui.add_space(4.0);
            for (t, title, sub) in [
                (Task::Repair, "Réparer rJb", "Restaure les fichiers, les raccourcis et le démarrage automatique. Vos réglages sont gardés."),
                (Task::UninstallApp, "Désinstaller l'application", "Supprime rJb mais conserve vos profils et réglages pour une prochaine installation."),
                (Task::UninstallAll, "Tout désinstaller", "Supprime rJb et tout ce qu'il a créé : réglages, profils, journaux."),
            ] {
                if choice(ui, self.task == t, title, sub) {
                    self.task = t;
                }
                ui.add_space(4.0);
            }
        });
        ui.add_space(8.0);
        note(ui, "Le pilote PawnIO, partagé avec d'autres logiciels, n'est pas retiré (Paramètres › Applications si besoin).");
    }

    fn maintenance_actions(&mut self, ui: &mut Ui, ctx: &egui::Context) {
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            let label = if self.task == Task::Repair { "Réparer" } else { "Désinstaller" };
            if primary(ui, label, true) {
                self.start(self.task, ctx);
            }
            if secondary(ui, "Annuler") {
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
        });
    }

    fn working_page(&mut self, ui: &mut Ui, task: Task) {
        let (step, done) = {
            let p = self.progress.lock().unwrap();
            (p.step.clone(), p.result.is_some())
        };
        if done {
            self.page = Page::Done(task);
            return;
        }
        card(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.add_space(30.0);
            ui.vertical_centered(|ui| {
                ui.add(egui::Spinner::new().size(36.0).color(ACCENT));
                ui.add_space(12.0);
                ui.label(RichText::new(trd(if step.is_empty() { "Préparation…" } else { &step })).size(15.0));
            });
            ui.add_space(30.0);
        });
    }

    fn result(&self) -> Result<Vec<String>, String> {
        self.progress.lock().unwrap().result.clone().unwrap_or(Ok(Vec::new()))
    }

    fn done_page(&mut self, ui: &mut Ui, task: Task) {
        let result = self.result();
        let uninstalling = matches!(task, Task::UninstallApp | Task::UninstallAll);
        card(ui, |ui| {
            ui.set_width(ui.available_width());
            match &result {
                Ok(notes) => {
                    let title = match task {
                        Task::Install => "rJb est installé ✔",
                        Task::Repair => "rJb est réparé ✔",
                        Task::UninstallApp => "rJb est désinstallé ✔",
                        Task::UninstallAll => "rJb et ses réglages sont supprimés ✔",
                    };
                    ui.label(RichText::new(tr(title)).font(bold(20.0)).color(GREEN));
                    ui.add_space(6.0);
                    if task == Task::UninstallApp {
                        note(ui, "Vos réglages sont conservés : ils seront retrouvés si vous réinstallez rJb.");
                    }
                    if task == Task::UninstallAll {
                        note(ui, "Plus aucun fichier de rJb ne reste sur ce PC.");
                    }
                    if !uninstalling {
                        note(ui, "Fermez G HUB, iCUE, Armoury Crate, MSI Center… pour que rJb puisse piloter vos appareils.");
                        note(ui, "Pour l'épingler : clic droit sur rJb dans le menu Démarrer › Épingler à la barre des tâches.");
                    }
                    for n in notes {
                        ui.label(RichText::new(format!("⚠ {}", trd(n))).color(Color32::from_rgb(255, 196, 90)).size(12.5));
                    }
                    if !uninstalling {
                        ui.add_space(6.0);
                        ui.horizontal(|ui| {
                            toggle(ui, &mut self.launch, "Lancer rJb maintenant");
                        });
                    }
                }
                Err(e) => {
                    ui.label(RichText::new(tr("L'opération n'a pas abouti")).font(bold(20.0)).color(Color32::from_rgb(255, 120, 120)));
                    ui.label(trd(e));
                }
            }
        });
    }

    fn done_actions(&mut self, ui: &mut Ui, ctx: &egui::Context, task: Task) {
        let result = self.result();
        let uninstalling = matches!(task, Task::UninstallApp | Task::UninstallAll);
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            if primary(ui, "Terminer", true) {
                if result.is_ok() {
                    if !uninstalling && self.launch {
                        let dir = if task == Task::Install {
                            self.opts.dir.clone()
                        } else {
                            install::installed_dir().unwrap_or_default()
                        };
                        let _ = std::process::Command::new(dir.join(install::APP_EXE)).current_dir(&dir).spawn();
                    }
                }
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
            if result.is_err() && secondary(ui, "Réessayer") {
                self.page = Page::Choose;
            }
        });
    }
}

impl eframe::App for SetupApp {
    fn clear_color(&self, _v: &egui::Visuals) -> [f32; 4] {
        if self.glass {
            [0.0; 4]
        } else {
            [0.05, 0.06, 0.10, 1.0]
        }
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = &ui.ctx().clone();
        // pendant le travail, on ne permet pas de fermer la fenêtre au milieu
        if matches!(self.page, Page::Working(_)) && ctx.input(|i| i.viewport().close_requested()) {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
        }
        // boutons toujours visibles en bas ; le contenu défile si la fenêtre
        // est plus petite que prévu (mise à l'échelle Windows élevée)
        egui::Panel::bottom("actions")
            .frame(Frame::NONE.inner_margin(Margin { left: 22, right: 22, top: 10, bottom: 18 }))
            .show_separator_line(false)
            .show(ui, |ui| match self.page {
                Page::Choose => match self.mode {
                    Mode::Setup => self.setup_actions(ui, ctx),
                    Mode::Maintenance => self.maintenance_actions(ui, ctx),
                },
                Page::Working(_) => {}
                Page::Done(t) => self.done_actions(ui, ctx, t),
            });
        egui::CentralPanel::default().frame(Frame::NONE.inner_margin(Margin { left: 22, right: 22, top: 20, bottom: 0 })).show(ui, |ui| {
            self.background(ui);
            self.header(ui);
            egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| match self.page {
                Page::Choose => match self.mode {
                    Mode::Setup => self.setup_page(ui),
                    Mode::Maintenance => self.maintenance_page(ui),
                },
                Page::Working(t) => self.working_page(ui, t),
                Page::Done(t) => self.done_page(ui, t),
            });
        });
    }
}
