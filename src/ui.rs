// SPDX-License-Identifier: GPL-2.0-or-later — Copyright (C) 2026 jeremdevgit — voir LICENSE
//! Interface « liquid glass » (egui). Créée à l'ouverture, détruite à la fermeture.

use crate::config::Lighting;
use crate::effects::{EffectKind, EffectSpec};
use crate::engine::{default_lighting, Cmd, DeviceView, Shared};
use crate::model::*;
use crate::{handle_common, take_actions, Action};
use eframe::egui::{
    self, pos2, vec2, Align, Align2, Color32, CornerRadius, CursorIcon, FontData, FontDefinitions, FontFamily,
    FontId, Frame, Layout, Margin, Mesh, Pos2, Rect, RichText, Sense, Shadow, Stroke, StrokeKind, TextureHandle, Ui,
};
use std::sync::atomic::Ordering;
use std::sync::Arc;

// ── palette ──────────────────────────────────────────────────────────────
pub(crate) const TEXT: Color32 = Color32::from_rgb(238, 241, 248);
pub(crate) const DIM: Color32 = Color32::from_rgb(150, 160, 182);
pub(crate) const ACCENT: Color32 = Color32::from_rgb(124, 92, 255);
pub(crate) const PINK: Color32 = Color32::from_rgb(255, 46, 99);
pub(crate) const CYAN: Color32 = Color32::from_rgb(0, 212, 255);
pub(crate) const GREEN: Color32 = Color32::from_rgb(40, 230, 140);

pub(crate) fn glass(a: u8) -> Color32 {
    Color32::from_rgba_unmultiplied(255, 255, 255, a)
}

fn c32(c: Color) -> Color32 {
    Color32::from_rgb(c.r, c.g, c.b)
}

pub fn run(shared: Arc<Shared>) {
    shared.ui_visible.store(true, Ordering::Relaxed);
    shared.send(Cmd::RefreshPreview);
    let icon = crate::icon::render_icon(64);
    let opts = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("rJb")
            .with_inner_size(crate::sys::fit_window(1180.0, 760.0))
            .with_min_inner_size([880.0, 560.0])
            .with_transparent(true)
            .with_icon(egui::IconData {
                rgba: icon,
                width: 64,
                height: 64,
            }),
        run_and_return: true,
        centered: true,
        ..Default::default()
    };
    let s = shared.clone();
    let _ = eframe::run_native("rJb", opts, Box::new(move |cc| Ok(Box::new(App::new(cc, s)))));
    crate::set_ui_ctx(None);
    shared.ui_visible.store(false, Ordering::Relaxed);
}

#[derive(Clone, PartialEq)]
enum Sel {
    All,
    Device(String),
    Settings,
    Help,
}

struct App {
    shared: Arc<Shared>,
    sel: Sel,
    zone_sel: Option<usize>,
    brush: Color,
    sync_spec: EffectSpec,
    new_profile: String,
    logo: TextureHandle,
    glass: bool,
    /// Résultat de la dernière action des réglages (export, import…).
    status: Option<(bool, String)>,
    /// Logiciels RGB concurrents détectés à l'ouverture de l'aide.
    conflicts: Option<Vec<&'static str>>,
}

impl App {
    fn new(cc: &eframe::CreationContext<'_>, shared: Arc<Shared>) -> Self {
        crate::set_ui_ctx(Some(cc.egui_ctx.clone()));
        let glass = apply_backdrop(cc);
        setup_style(&cc.egui_ctx);
        let rgba = crate::icon::render_icon(96);
        let logo = cc.egui_ctx.load_texture(
            "logo",
            egui::ColorImage::from_rgba_unmultiplied([96, 96], &rgba),
            egui::TextureOptions::LINEAR,
        );
        let first = shared
            .devices
            .read()
            .unwrap()
            .first()
            .map(|d| Sel::Device(d.info.id.clone()))
            .unwrap_or(Sel::All);
        // l'état réel du démarrage automatique (il a pu être changé ailleurs)
        let auto = crate::install::autostart_enabled();
        shared.config.lock().unwrap().autostart = auto;
        App {
            status: None,
            conflicts: None,
            shared,
            sel: first,
            zone_sel: None,
            brush: Color::rgb(124, 92, 255),
            sync_spec: EffectSpec::default(),
            new_profile: String::new(),
            logo,
            glass,
        }
    }
}

pub(crate) fn apply_backdrop(cc: &eframe::CreationContext<'_>) -> bool {
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};
    use windows_sys::Win32::Graphics::Dwm::*;
    use windows_sys::Win32::UI::Controls::MARGINS;
    let Ok(h) = cc.window_handle() else { return false };
    let RawWindowHandle::Win32(h) = h.as_raw() else { return false };
    let hwnd = h.hwnd.get() as *mut core::ffi::c_void;
    unsafe {
        let dark: i32 = 1;
        DwmSetWindowAttribute(hwnd, DWMWA_USE_IMMERSIVE_DARK_MODE as u32, &dark as *const _ as _, 4);
        let corner: i32 = 2; // DWMWCP_ROUND
        DwmSetWindowAttribute(hwnd, DWMWA_WINDOW_CORNER_PREFERENCE as u32, &corner as *const _ as _, 4);
        let backdrop: i32 = DWMSBT_TRANSIENTWINDOW; // acrylique
        let ok = DwmSetWindowAttribute(hwnd, DWMWA_SYSTEMBACKDROP_TYPE as u32, &backdrop as *const _ as _, 4) == 0;
        if ok {
            let m = MARGINS {
                cxLeftWidth: -1,
                cxRightWidth: -1,
                cyTopHeight: -1,
                cyBottomHeight: -1,
            };
            DwmExtendFrameIntoClientArea(hwnd, &m);
        }
        ok
    }
}

pub(crate) fn setup_style(ctx: &egui::Context) {
    let mut fonts = FontDefinitions::default();
    let dir = std::env::var("WINDIR").unwrap_or_else(|_| "C:\\Windows".into()) + "\\Fonts\\";
    let mut add = |name: &str, files: &[&str], family: FontFamily, front: bool| {
        for f in files {
            if let Ok(b) = std::fs::read(format!("{dir}{f}")) {
                fonts.font_data.insert(name.into(), Arc::new(FontData::from_owned(b)));
                let fam = fonts.families.entry(family.clone()).or_default();
                if front {
                    fam.insert(0, name.into());
                } else {
                    fam.push(name.into());
                }
                return;
            }
        }
    };
    add("ui", &["SegUIVar.ttf", "segoeui.ttf"], FontFamily::Proportional, true);
    add("sym", &["seguisym.ttf"], FontFamily::Proportional, false);
    add("bold", &["seguisb.ttf", "segoeuib.ttf"], FontFamily::Name("bold".into()), true);
    let prop = fonts.families[&FontFamily::Proportional].clone();
    fonts
        .families
        .entry(FontFamily::Name("bold".into()))
        .or_default()
        .extend(prop);
    ctx.set_fonts(fonts);

    let mut v = egui::Visuals::dark();
    v.panel_fill = Color32::TRANSPARENT;
    v.window_fill = Color32::from_rgba_unmultiplied(22, 24, 36, 240);
    v.window_stroke = Stroke::new(1.0_f32,glass(30));
    v.window_corner_radius = CornerRadius::same(16);
    v.menu_corner_radius = CornerRadius::same(12);
    v.extreme_bg_color = Color32::from_rgba_unmultiplied(0, 0, 0, 70);
    v.faint_bg_color = glass(8);
    v.selection.bg_fill = ACCENT;
    v.selection.stroke = Stroke::new(1.0_f32,TEXT);
    v.slider_trailing_fill = true;
    v.override_text_color = Some(TEXT);
    v.hyperlink_color = CYAN;
    for (w, a) in [
        (&mut v.widgets.noninteractive, 0u8),
        (&mut v.widgets.inactive, 16),
        (&mut v.widgets.hovered, 30),
        (&mut v.widgets.active, 44),
        (&mut v.widgets.open, 30),
    ] {
        w.bg_fill = glass(a);
        w.weak_bg_fill = glass(a);
        w.corner_radius = CornerRadius::same(10);
        w.bg_stroke = Stroke::new(1.0_f32,glass(if a == 0 { 18 } else { a / 2 + 14 }));
        w.fg_stroke = Stroke::new(1.0_f32,TEXT);
    }
    v.widgets.noninteractive.fg_stroke = Stroke::new(1.0_f32,DIM);
    ctx.set_visuals(v);
    ctx.style_mut(|s| {
        s.spacing.item_spacing = vec2(8.0, 8.0);
        s.spacing.button_padding = vec2(12.0, 6.0);
        s.spacing.slider_width = 260.0;
        s.spacing.interact_size.y = 28.0;
        s.text_styles.insert(egui::TextStyle::Body, FontId::proportional(14.5));
        s.text_styles.insert(egui::TextStyle::Button, FontId::proportional(14.0));
        s.text_styles.insert(egui::TextStyle::Small, FontId::proportional(12.0));
        s.text_styles.insert(egui::TextStyle::Heading, FontId::new(22.0, FontFamily::Name("bold".into())));
    });
}

// ── éléments graphiques ─────────────────────────────────────────────────

pub(crate) fn bold(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name("bold".into()))
}

/// Carte en verre : fond translucide, liseré clair, reflet en haut, ombre douce.
pub(crate) fn card<R>(ui: &mut Ui, add: impl FnOnce(&mut Ui) -> R) -> R {
    let r = Frame::new()
        .fill(glass(13))
        .stroke(Stroke::new(1.0_f32,glass(28)))
        .corner_radius(CornerRadius::same(18))
        .inner_margin(Margin::same(16))
        .shadow(Shadow {
            offset: [0, 10],
            blur: 28,
            spread: 0,
            color: Color32::from_black_alpha(55),
        })
        .show(ui, add);
    let rect = r.response.rect;
    // reflet supérieur du verre
    let p = ui.painter();
    let y = rect.top() + 1.0;
    let mut mesh = Mesh::default();
    let l = rect.left() + 18.0;
    let rr = rect.right() - 18.0;
    let mid = (l + rr) / 2.0;
    for (x0, x1, a0, a1) in [(l, mid, 0u8, 70u8), (mid, rr, 70, 0)] {
        let i = mesh.vertices.len() as u32;
        mesh.colored_vertex(pos2(x0, y), glass(a0));
        mesh.colored_vertex(pos2(x1, y), glass(a1));
        mesh.colored_vertex(pos2(x1, y + 1.2), glass(a1 / 3));
        mesh.colored_vertex(pos2(x0, y + 1.2), glass(a0 / 3));
        mesh.add_triangle(i, i + 1, i + 2);
        mesh.add_triangle(i, i + 2, i + 3);
    }
    p.add(mesh);
    r.inner
}

/// Halo lumineux radial (dégradé vers transparent).
pub(crate) fn glow(p: &egui::Painter, c: Pos2, r: f32, col: Color32) {
    let mut mesh = Mesh::default();
    let n = 48;
    mesh.colored_vertex(c, col);
    let [cr, cg, cb, _] = col.to_srgba_unmultiplied();
    let edge = Color32::from_rgba_unmultiplied(cr, cg, cb, 0);
    for i in 0..=n {
        let a = i as f32 / n as f32 * std::f32::consts::TAU;
        mesh.colored_vertex(c + vec2(a.cos(), a.sin()) * r, edge);
    }
    for i in 1..=n as u32 {
        mesh.add_triangle(0, i, i + 1);
    }
    p.add(mesh);
}

pub(crate) fn chip(ui: &mut Ui, selected: bool, text: &str) -> egui::Response {
    let font = FontId::proportional(13.5);
    let galley = ui.painter().layout_no_wrap(text.to_string(), font, TEXT);
    let size = vec2(galley.size().x + 26.0, 30.0);
    let (rect, resp) = ui.allocate_exact_size(size, Sense::click());
    let p = ui.painter();
    let cr = CornerRadius::same(15);
    if selected {
        p.rect_filled(rect, cr, Color32::from_rgba_unmultiplied(124, 92, 255, 150));
        p.rect_stroke(rect, cr, Stroke::new(1.0_f32,glass(90)), StrokeKind::Inside);
    } else {
        let a = if resp.hovered() { 30 } else { 14 };
        p.rect_filled(rect, cr, glass(a));
        p.rect_stroke(rect, cr, Stroke::new(1.0_f32,glass(26)), StrokeKind::Inside);
    }
    p.galley(rect.center() - galley.size() / 2.0, galley, TEXT);
    resp.on_hover_cursor(CursorIcon::PointingHand)
}

/// Texte sur une ligne, tronqué avec « … ».
fn one_line(t: &str, font: FontId, color: Color32, width: f32) -> egui::text::LayoutJob {
    let mut job = egui::text::LayoutJob::simple_singleline(t.to_string(), font, color);
    job.wrap = egui::text::TextWrapping::truncate_at_width(width);
    job
}

pub(crate) fn label_dim(ui: &mut Ui, t: &str) {
    ui.label(RichText::new(t).color(DIM).size(12.5));
}

pub(crate) fn section(ui: &mut Ui, t: &str) {
    ui.add_space(4.0);
    ui.label(RichText::new(t.to_uppercase()).color(DIM).size(11.5).font(bold(11.5)));
}

fn slider(ui: &mut Ui, label: &str, v: &mut f32) -> bool {
    let mut changed = false;
    ui.horizontal(|ui| {
        ui.add_sized([110.0, 24.0], egui::Label::new(RichText::new(label).color(DIM)));
        changed = ui
            .add(egui::Slider::new(v, 0.0..=1.0).show_value(false))
            .changed();
        ui.label(RichText::new(format!("{:.0} %", *v * 100.0)).color(DIM).size(12.5));
    });
    changed
}

pub(crate) fn toggle(ui: &mut Ui, on: &mut bool, label: &str) -> bool {
    let (rect, resp) = ui.allocate_exact_size(vec2(40.0, 22.0), Sense::click());
    if resp.clicked() {
        *on = !*on;
    }
    let t = ui.ctx().animate_bool_responsive(resp.id, *on);
    let p = ui.painter();
    let bg = Color32::from_rgba_unmultiplied(124, 92, 255, (40.0 + 160.0 * t) as u8);
    p.rect_filled(rect, CornerRadius::same(11), bg);
    p.rect_stroke(rect, CornerRadius::same(11), Stroke::new(1.0_f32,glass(50)), StrokeKind::Inside);
    let x = egui::lerp(rect.left() + 11.0..=rect.right() - 11.0, t);
    p.circle_filled(pos2(x, rect.center().y), 8.0, TEXT);
    ui.label(label);
    resp.clicked()
}

fn kind_glyph(k: DeviceKind) -> &'static str {
    match k {
        DeviceKind::Keyboard => "⌨",
        DeviceKind::Mouse => "🖱",
        DeviceKind::Mousemat => "▭",
        DeviceKind::Headset | DeviceKind::HeadsetStand => "🎧",
        DeviceKind::Speaker => "🔊",
        DeviceKind::Microphone => "🎤",
        DeviceKind::Motherboard => "▦",
        DeviceKind::Ram => "▤",
        DeviceKind::Gpu => "▣",
        DeviceKind::Cooler => "❄",
        DeviceKind::FanController => "✇",
        DeviceKind::LedStrip => "〰",
        DeviceKind::Case => "▯",
        DeviceKind::Gamepad => "🎮",
        DeviceKind::Light => "💡",
        DeviceKind::Other => "◆",
    }
}

fn average(colors: &[Color]) -> Color32 {
    if colors.is_empty() {
        return glass(40);
    }
    let (mut r, mut g, mut b) = (0u32, 0u32, 0u32);
    for c in colors {
        r += c.r as u32;
        g += c.g as u32;
        b += c.b as u32;
    }
    let n = colors.len() as u32;
    Color32::from_rgb((r / n) as u8, (g / n) as u8, (b / n) as u8)
}

/// Couleurs éditables (min..max), retourne true si modifiées.
fn colors_editor(ui: &mut Ui, colors: &mut Vec<Color>, min: u8, max: u8) -> bool {
    let mut changed = false;
    while colors.len() < min as usize {
        colors.push(Color::from_hsv(colors.len() as f32 * 110.0 + 330.0, 0.85, 1.0));
        changed = true;
    }
    ui.horizontal_wrapped(|ui| {
        ui.add_sized([110.0, 24.0], egui::Label::new(RichText::new("Couleurs").color(DIM)));
        let mut remove = None;
        for (i, c) in colors.iter_mut().take(max as usize).enumerate() {
            let mut rgb = [c.r, c.g, c.b];
            let r = egui::color_picker::color_edit_button_srgb(ui, &mut rgb);
            if r.changed() {
                *c = Color::rgb(rgb[0], rgb[1], rgb[2]);
                changed = true;
            }
            if r.secondary_clicked() {
                remove = Some(i);
            }
        }
        if let Some(i) = remove {
            if colors.len() > min as usize {
                colors.remove(i);
                changed = true;
            }
        }
        if colors.len() > max as usize {
            colors.truncate(max as usize);
        }
        if colors.len() < max as usize && ui.button("＋").on_hover_text("Ajouter une couleur").clicked() {
            let h = colors.len() as f32 * 67.0 + 200.0;
            colors.push(Color::from_hsv(h, 0.9, 1.0));
            changed = true;
        }
        if max > 1 && colors.len() > min.max(1) as usize {
            label_dim(ui, "clic droit sur une couleur pour la retirer");
        }
    });
    changed
}

fn direction_combo(ui: &mut Ui, d: &mut Direction, allowed: &[Direction]) -> bool {
    let mut changed = false;
    ui.horizontal(|ui| {
        ui.add_sized([110.0, 24.0], egui::Label::new(RichText::new("Direction").color(DIM)));
        egui::ComboBox::from_id_salt("dir")
            .selected_text(d.label())
            .width(240.0)
            .show_ui(ui, |ui| {
                for x in allowed {
                    changed |= ui.selectable_value(d, *x, x.label()).changed();
                }
            });
    });
    changed
}

/// Éditeur d'un effet rJb.
fn effect_editor(ui: &mut Ui, s: &mut EffectSpec) -> bool {
    let mut changed = false;
    section(ui, "Effet");
    ui.horizontal_wrapped(|ui| {
        for k in EffectKind::ALL {
            if chip(ui, s.kind == k, k.label()).clicked() && s.kind != k {
                s.kind = k;
                changed = true;
            }
        }
    });
    ui.add_space(6.0);
    let u = s.kind.uses();
    if s.kind == EffectKind::Off {
        return changed;
    }
    section(ui, "Réglages");
    if u.speed {
        changed |= slider(ui, "Vitesse", &mut s.speed);
    }
    changed |= slider(ui, "Luminosité", &mut s.brightness);
    if u.size {
        let name = match s.kind {
            EffectKind::Twinkle => "Densité",
            EffectKind::Comet => "Longueur",
            EffectKind::Fire | EffectKind::Aurora => "Échelle",
            _ => "Taille",
        };
        changed |= slider(ui, name, &mut s.size);
    }
    if u.direction {
        changed |= direction_combo(ui, &mut s.direction, &Direction::ALL);
    }
    if u.random {
        ui.horizontal(|ui| {
            ui.add_space(118.0);
            changed |= toggle(ui, &mut s.random, "Couleurs aléatoires");
        });
    }
    if u.colors.1 > 0 && !(u.random && s.random) {
        changed |= colors_editor(ui, &mut s.colors, u.colors.0, u.colors.1);
    }
    changed
}

/// Éditeur d'un effet matériel.
fn hardware_editor(ui: &mut Ui, info: &DeviceInfo, mode: &mut String, p: &mut HwParams) -> bool {
    let mut changed = false;
    section(ui, "Effet intégré à l'appareil");
    ui.horizontal_wrapped(|ui| {
        for m in &info.modes {
            if chip(ui, *mode == m.name, &m.name).clicked() && *mode != m.name {
                *mode = m.name.clone();
                changed = true;
            }
        }
    });
    let Some(m) = info.modes.iter().find(|m| m.name == *mode) else {
        return changed;
    };
    if m.is_off {
        return changed;
    }
    ui.add_space(6.0);
    section(ui, "Réglages");
    if m.speed {
        changed |= slider(ui, "Vitesse", &mut p.speed);
    }
    if m.brightness {
        changed |= slider(ui, "Luminosité", &mut p.brightness);
    }
    if !m.directions.is_empty() {
        if !m.directions.contains(&p.direction) {
            p.direction = m.directions[0];
        }
        changed |= direction_combo(ui, &mut p.direction, &m.directions);
    }
    if m.random {
        ui.horizontal(|ui| {
            ui.add_space(118.0);
            changed |= toggle(ui, &mut p.random, "Couleurs aléatoires");
        });
    }
    if m.colors.1 > 0 && !(m.random && p.random) {
        changed |= colors_editor(ui, &mut p.colors, m.colors.0, m.colors.1);
    }
    ui.add_space(4.0);
    label_dim(ui, "Les effets intégrés continuent sans rJb, même PC éteint selon l'appareil.");
    changed
}

/// Aperçu des LED. Renvoie (zone, led) cliquée.
fn preview(ui: &mut Ui, info: &DeviceInfo, colors: &[Color]) -> Option<(usize, usize)> {
    let width = ui.available_width();
    let mut clicked = None;
    let mut off = 0;
    for (zi, zone) in info.zones.iter().enumerate() {
        let n = zone.leds.len();
        let zc = &colors[off.min(colors.len())..(off + n).min(colors.len())];
        if info.zones.len() > 1 || zone.grid.is_none() {
            label_dim(ui, &format!("{} · {} LED", zone.name, n));
        }
        let (h, keyboard) = match zone.grid {
            Some((gw, gh)) => (width / gw as f32 * gh as f32, true),
            None => (34.0, false),
        };
        let (rect, resp) = ui.allocate_exact_size(vec2(width, h.min(300.0)), Sense::click_and_drag());
        let p = ui.painter_at(rect.expand(12.0));
        let pos_of = |l: &Led| -> Pos2 {
            if keyboard {
                pos2(rect.left() + l.x * rect.width(), rect.top() + l.y * rect.height())
            } else {
                let pad = 16.0;
                pos2(rect.left() + pad + l.x * (rect.width() - 2.0 * pad), rect.center().y)
            }
        };
        let unit = match zone.grid {
            Some((gw, gh)) => (rect.width() / gw as f32).min(rect.height() / gh as f32),
            None => ((rect.width() - 32.0) / n.max(1) as f32).clamp(3.0, 26.0),
        };
        for (i, l) in zone.leds.iter().enumerate() {
            let c = zc.get(i).copied().unwrap_or(Color::BLACK);
            let col = c32(c);
            let center = pos_of(l);
            let lum = c.max_channel();
            if keyboard {
                let r = Rect::from_center_size(center, vec2(unit * l.w - 3.0, unit - 3.0));
                if lum > 8 {
                    glow(&p, center, unit * 0.9, col.gamma_multiply(0.35));
                }
                p.rect_filled(r, CornerRadius::same(5), Color32::from_rgba_unmultiplied(10, 12, 20, 160));
                p.rect_filled(r.shrink(1.5), CornerRadius::same(4), col.gamma_multiply(0.85));
                p.rect_stroke(r, CornerRadius::same(5), Stroke::new(1.0_f32,glass(30)), StrokeKind::Inside);
            } else {
                let rad = (unit * 0.42).clamp(1.5, 11.0);
                if lum > 8 {
                    glow(&p, center, rad * 2.6, col.gamma_multiply(0.45));
                }
                p.circle_filled(center, rad, col);
                p.circle_stroke(center, rad, Stroke::new(1.0_f32,glass(40)));
            }
        }
        if let Some(pt) = resp.interact_pointer_pos() {
            if resp.clicked() || resp.dragged() {
                let best = zone
                    .leds
                    .iter()
                    .enumerate()
                    .min_by(|a, b| {
                        pos_of(a.1).distance_sq(pt).total_cmp(&pos_of(b.1).distance_sq(pt))
                    })
                    .map(|(i, _)| i);
                if let Some(i) = best {
                    clicked = Some((zi, i));
                }
            }
        }
        if let Some(pt) = resp.hover_pos() {
            if let Some(l) = zone
                .leds
                .iter()
                .min_by(|a, b| pos_of(a).distance_sq(pt).total_cmp(&pos_of(b).distance_sq(pt)))
            {
                if pos_of(l).distance(pt) < unit {
                    resp.on_hover_text(&l.name);
                }
            }
        }
        off += n;
        ui.add_space(4.0);
    }
    clicked
}

// ── application ─────────────────────────────────────────────────────────

impl eframe::App for App {
    fn clear_color(&self, _v: &egui::Visuals) -> [f32; 4] {
        if self.glass {
            [0.0, 0.0, 0.0, 0.0]
        } else {
            [0.05, 0.06, 0.10, 1.0]
        }
    }

    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        for a in take_actions() {
            match a {
                Action::Open => {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(false));
                    ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
                }
                Action::Quit => {
                    crate::post(Action::Quit);
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                }
                Action::RebuildTray => {}
                other => handle_common(&self.shared, other),
            }
        }

        let devices: Vec<DeviceView> = self.shared.devices.read().unwrap().clone();
        let mut changed: Vec<String> = Vec::new();
        let mut reload = false;
        let mut animated = false;

        egui::CentralPanel::default()
            .frame(Frame::NONE.inner_margin(Margin::same(14)))
            .show(ctx, |ui| {
                self.background(ui);
                self.top_bar(ui, &mut reload);
                ui.add_space(10.0);
                ui.horizontal_top(|ui| {
                    let h = ui.available_height();
                    ui.allocate_ui_with_layout(vec2(272.0, h), Layout::top_down(Align::Min), |ui| {
                        self.sidebar(ui, &devices);
                    });
                    ui.add_space(6.0);
                    ui.allocate_ui_with_layout(ui.available_size(), Layout::top_down(Align::Min), |ui| {
                        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                            match self.sel.clone() {
                                Sel::Settings => self.settings(ui, &mut reload),
                                Sel::Help => self.help(ui),
                                Sel::All => self.all_page(ui, &devices, &mut changed),
                                Sel::Device(id) => match devices.iter().find(|d| d.info.id == id) {
                                    Some(d) => animated |= self.device_page(ui, d, &mut changed),
                                    None => {
                                        self.sel = Sel::All;
                                    }
                                },
                            }
                        });
                    });
                });
            });

        // animation de l'aperçu seulement si un effet bouge
        {
            let c = self.shared.config.lock().unwrap();
            if !c.all_off {
                animated |= c.profile().devices.values().any(|l| match l {
                    Lighting::Software { zones } => zones.iter().any(|z| z.kind.animated()),
                    _ => false,
                });
            }
        }
        if animated {
            ctx.request_repaint_after(std::time::Duration::from_millis(33));
        }

        if reload {
            self.shared.send(Cmd::ReloadAll);
            crate::post(Action::RebuildTray);
        }
        for id in changed {
            self.shared.send(Cmd::Changed(id));
        }
    }
}

impl App {
    fn background(&self, ui: &mut Ui) {
        let r = ui.ctx().screen_rect();
        let p = ui.ctx().layer_painter(egui::LayerId::background());
        if self.glass {
            p.rect_filled(r, CornerRadius::ZERO, Color32::from_rgba_unmultiplied(10, 11, 20, 150));
        } else {
            p.rect_filled(r, CornerRadius::ZERO, Color32::from_rgb(13, 15, 26));
        }
        // reflets RGB « liquides »
        glow(&p, pos2(r.left() + r.width() * 0.08, r.top() + r.height() * 0.1), r.width() * 0.32, Color32::from_rgba_unmultiplied(255, 46, 99, 38));
        glow(&p, pos2(r.right() - r.width() * 0.05, r.top() + r.height() * 0.25), r.width() * 0.36, Color32::from_rgba_unmultiplied(0, 170, 255, 34));
        glow(&p, pos2(r.left() + r.width() * 0.45, r.bottom() + r.height() * 0.05), r.width() * 0.38, Color32::from_rgba_unmultiplied(40, 230, 140, 26));
        glow(&p, pos2(r.center().x, r.top() + r.height() * 0.45), r.width() * 0.25, Color32::from_rgba_unmultiplied(124, 92, 255, 22));
    }

    fn top_bar(&mut self, ui: &mut Ui, reload: &mut bool) {
        ui.horizontal(|ui| {
            ui.add(egui::Image::new(&self.logo).fit_to_exact_size(vec2(38.0, 38.0)));
            ui.vertical(|ui| {
                ui.add_space(1.0);
                ui.label(RichText::new("rJb").font(bold(22.0)));
                ui.add_space(-6.0);
                label_dim(ui, "Contrôle RGB unifié");
            });
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                let help = self.sel == Sel::Help;
                if chip(ui, help, "?  Aide").clicked() {
                    self.sel = if help { Sel::All } else { Sel::Help };
                    self.conflicts = None;
                }
                let set = self.sel == Sel::Settings;
                if chip(ui, set, "⚙  Réglages").clicked() {
                    self.sel = if set { Sel::All } else { Sel::Settings };
                    self.status = None;
                }
                let mut cfg = self.shared.config.lock().unwrap();
                let mut off = cfg.all_off;
                if chip(ui, off, if off { "⏻  Éclairage coupé" } else { "⏻  Tout éteindre" }).clicked() {
                    off = !off;
                    cfg.all_off = off;
                    *reload = true;
                }
                ui.add_space(6.0);
                let mut b = cfg.brightness;
                if ui
                    .add(egui::Slider::new(&mut b, 0.0..=1.0).show_value(false))
                    .on_hover_text("Luminosité générale")
                    .changed()
                {
                    cfg.brightness = b;
                    *reload = true;
                }
                ui.label(RichText::new("☀").size(16.0).color(DIM));
                ui.add_space(10.0);
                // profils
                let active = cfg.active_profile.clone();
                let names: Vec<String> = cfg.profiles.keys().cloned().collect();
                egui::ComboBox::from_id_salt("profile")
                    .selected_text(format!("◈  {active}"))
                    .width(170.0)
                    .show_ui(ui, |ui| {
                        for n in names {
                            if ui.selectable_label(n == active, &n).clicked() && n != active {
                                cfg.active_profile = n;
                                *reload = true;
                            }
                        }
                    });
            });
        });
    }

    fn sidebar(&mut self, ui: &mut Ui, devices: &[DeviceView]) {
        card(ui, |ui| {
            ui.set_min_height(ui.available_height() - 34.0);
            ui.set_width(240.0);
            section(ui, "Appareils");
            if self.side_item(ui, self.sel == Sel::All, "✦", "Tous les appareils", "Synchroniser", Color32::TRANSPARENT) {
                self.sel = Sel::All;
            }
            ui.add_space(2.0);
            egui::ScrollArea::vertical().id_salt("devs").max_height(ui.available_height() - 80.0).show(ui, |ui| {
                for d in devices {
                    let sel = self.sel == Sel::Device(d.info.id.clone());
                    let sub = if d.error.is_some() {
                        "⚠ erreur de communication".to_string()
                    } else {
                        format!("{} · {}", d.info.vendor, d.info.kind.label())
                    };
                    if self.side_item(ui, sel, kind_glyph(d.info.kind), &d.info.name, &sub, average(&d.colors)) {
                        self.sel = Sel::Device(d.info.id.clone());
                        self.zone_sel = None;
                    }
                }
                if devices.is_empty() {
                    ui.add_space(8.0);
                    label_dim(ui, "Aucun appareil trouvé pour l'instant.");
                }
            });
            ui.with_layout(Layout::bottom_up(Align::Min), |ui| {
                let scanning = self.shared.scanning.load(Ordering::Relaxed);
                ui.horizontal(|ui| {
                    if ui.button("⟳  Rechercher").clicked() {
                        self.shared.send(Cmd::Rescan);
                    }
                    if scanning {
                        ui.spinner();
                    }
                });
                let notices = self.shared.notices.lock().unwrap().clone();
                for n in notices.iter().rev() {
                    if n == crate::drivers::ADMIN_NOTICE && ui.button("🛡  Redémarrer en administrateur").clicked() {
                        crate::relaunch_elevated();
                    }
                    if n.contains("PawnIO") && ui.button("⬇  Installer PawnIO").on_hover_text("Pilote signé requis pour la RAM et la carte mère").clicked() {
                        install_pawnio();
                    }
                    match n.strip_prefix(crate::drivers::INFO_PREFIX) {
                        Some(info) => ui.label(RichText::new(info).weak().size(12.0)),
                        None => ui.label(RichText::new(format!("⚠ {n}")).color(Color32::from_rgb(255, 196, 90)).size(12.0)),
                    };
                }
            });
        });
    }

    fn side_item(&self, ui: &mut Ui, selected: bool, glyph: &str, title: &str, sub: &str, dot: Color32) -> bool {
        let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 50.0), Sense::click());
        let p = ui.painter();
        let cr = CornerRadius::same(12);
        if selected {
            p.rect_filled(rect, cr, Color32::from_rgba_unmultiplied(124, 92, 255, 70));
            p.rect_stroke(rect, cr, Stroke::new(1.0_f32,Color32::from_rgba_unmultiplied(170, 150, 255, 110)), StrokeKind::Inside);
        } else if resp.hovered() {
            p.rect_filled(rect, cr, glass(16));
        }
        p.text(pos2(rect.left() + 22.0, rect.center().y), Align2::CENTER_CENTER, glyph, FontId::proportional(18.0), TEXT);
        let tw = rect.width() - 74.0;
        let title_g = p.layout_job(one_line(title, FontId::proportional(14.0), TEXT, tw));
        let sub_g = p.layout_job(one_line(sub, FontId::proportional(11.5), DIM, tw));
        p.galley(pos2(rect.left() + 42.0, rect.top() + 7.0), title_g, TEXT);
        p.galley(pos2(rect.left() + 42.0, rect.top() + 27.0), sub_g, DIM);
        if dot != Color32::TRANSPARENT {
            let c = pos2(rect.right() - 16.0, rect.center().y);
            glow(p, c, 13.0, dot.gamma_multiply(0.5));
            p.circle_filled(c, 5.0, dot);
        }
        resp.on_hover_cursor(CursorIcon::PointingHand).clicked()
    }

    /// Page d'un appareil. Retourne true si un effet animé est actif.
    fn device_page(&mut self, ui: &mut Ui, d: &DeviceView, changed: &mut Vec<String>) -> bool {
        let info = &d.info;
        let mut cfg = self.shared.config.lock().unwrap();
        let disabled = cfg.disabled.contains(&info.id);
        let lighting = cfg
            .profile_mut()
            .devices
            .entry(info.id.clone())
            .or_insert_with(|| default_lighting(info));
        let mut dirty = false;

        // en-tête + aperçu
        card(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                ui.label(RichText::new(kind_glyph(info.kind)).size(26.0));
                ui.vertical(|ui| {
                    ui.label(RichText::new(&info.name).font(bold(20.0)));
                    label_dim(ui, &format!("{} · {} · {} LED", info.vendor, info.kind.label(), info.led_count()));
                });
            });
            if let Some(e) = &d.error {
                ui.label(RichText::new(format!("⚠ {e}")).color(Color32::from_rgb(255, 120, 120)));
            }
            ui.add_space(8.0);
            if let Some((z, i)) = preview(ui, info, &d.colors) {
                // peinture LED par LED
                if let Lighting::Software { zones } = lighting {
                    fill_zones(zones, info);
                    let s = &mut zones[z];
                    if s.kind != EffectKind::Custom {
                        let base = s.colors.first().copied().unwrap_or(Color::WHITE);
                        s.kind = EffectKind::Custom;
                        s.custom = vec![base; info.zones[z].leds.len()];
                    }
                    s.custom.resize(info.zones[z].leds.len(), Color::BLACK);
                    if s.custom[i] != self.brush {
                        s.custom[i] = self.brush;
                        dirty = true;
                    }
                }
            }
        });
        ui.add_space(10.0);

        let mut animated = false;
        card(ui, |ui| {
            ui.set_width(ui.available_width());
            if disabled {
                label_dim(ui, "Cet appareil est ignoré par rJb (voir Réglages).");
                return;
            }
            // source
            ui.horizontal(|ui| {
                let sw = matches!(lighting, Lighting::Software { .. });
                if info.direct && chip(ui, sw, "✦  Effet rJb").clicked() && !sw {
                    *lighting = Lighting::default();
                    dirty = true;
                }
                if !info.modes.is_empty() && chip(ui, !sw, "◇  Effet intégré").clicked() && sw {
                    *lighting = Lighting::Hardware {
                        mode: info.modes[0].name.clone(),
                        params: HwParams::default(),
                    };
                    dirty = true;
                }
            });
            ui.add_space(6.0);
            match lighting {
                Lighting::Software { zones } => {
                    fill_zones(zones, info);
                    // zones
                    if info.zones.len() > 1 {
                        section(ui, "Zones");
                        ui.horizontal_wrapped(|ui| {
                            if chip(ui, self.zone_sel.is_none(), "Toutes").clicked() {
                                self.zone_sel = None;
                            }
                            for (i, z) in info.zones.iter().enumerate() {
                                if chip(ui, self.zone_sel == Some(i), &z.name).clicked() {
                                    self.zone_sel = Some(i);
                                }
                            }
                        });
                    }
                    let zi = self.zone_sel.filter(|&z| z < zones.len());
                    // nombre de LED des connecteurs ARGB
                    for (i, z) in info.zones.iter().enumerate() {
                        if zi.is_some_and(|s| s != i) {
                            continue;
                        }
                        if let Some((min, max)) = z.resizable {
                            ui.horizontal(|ui| {
                                ui.add_sized([110.0, 24.0], egui::Label::new(RichText::new(format!("LED · {}", z.name)).color(DIM)));
                                let mut n = z.leds.len() as u16;
                                if ui.add(egui::DragValue::new(&mut n).range(min..=max)).changed() {
                                    self.shared.send(Cmd::Resize { id: info.id.clone(), zone: i, leds: n });
                                }
                            });
                        }
                    }
                    let target = zi.unwrap_or(0);
                    let mut spec = zones[target].clone();
                    if effect_editor(ui, &mut spec) {
                        match zi {
                            Some(z) => zones[z] = spec.clone(),
                            None => {
                                for z in zones.iter_mut() {
                                    let custom = std::mem::take(&mut z.custom);
                                    *z = spec.clone();
                                    if z.custom.is_empty() {
                                        z.custom = custom;
                                    }
                                }
                            }
                        }
                        dirty = true;
                    }
                    if spec.kind == EffectKind::Custom {
                        ui.add_space(6.0);
                        section(ui, "Pinceau");
                        ui.horizontal(|ui| {
                            let mut rgb = [self.brush.r, self.brush.g, self.brush.b];
                            if egui::color_picker::color_edit_button_srgb(ui, &mut rgb).changed() {
                                self.brush = Color::rgb(rgb[0], rgb[1], rgb[2]);
                            }
                            label_dim(ui, "Cliquez ou glissez sur l'aperçu pour peindre les LED.");
                            if ui.button("Tout remplir").clicked() {
                                for (i, z) in zones.iter_mut().enumerate() {
                                    if zi.is_none_or(|s| s == i) {
                                        z.custom = vec![self.brush; info.zones[i].leds.len()];
                                    }
                                }
                                dirty = true;
                            }
                        });
                    }
                    animated = zones.iter().any(|z| z.kind.animated());
                }
                Lighting::Hardware { mode, params } => {
                    dirty |= hardware_editor(ui, info, mode, params);
                }
            }
        });
        if dirty {
            changed.push(info.id.clone());
        }
        drop(cfg);
        animated
    }

    fn all_page(&mut self, ui: &mut Ui, devices: &[DeviceView], changed: &mut Vec<String>) {
        card(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.label(RichText::new("Tous les appareils").font(bold(20.0)));
            label_dim(ui, "Un même effet, synchronisé sur l'ensemble de vos appareils.");
            ui.add_space(8.0);
            ui.horizontal_wrapped(|ui| {
                for d in devices {
                    let (r, _) = ui.allocate_exact_size(vec2(200.0, 34.0), Sense::hover());
                    let p = ui.painter();
                    p.rect_filled(r, CornerRadius::same(10), glass(12));
                    let c = average(&d.colors);
                    glow(p, pos2(r.left() + 16.0, r.center().y), 12.0, c.gamma_multiply(0.6));
                    p.circle_filled(pos2(r.left() + 16.0, r.center().y), 5.0, c);
                    let g = p.layout_job(one_line(&d.info.name, FontId::proportional(13.0), TEXT, r.width() - 40.0));
                    p.galley(pos2(r.left() + 30.0, r.center().y - g.size().y / 2.0), g, TEXT);
                }
            });
        });
        ui.add_space(10.0);
        card(ui, |ui| {
            ui.set_width(ui.available_width());
            effect_editor(ui, &mut self.sync_spec);
            ui.add_space(10.0);
            if ui
                .add(egui::Button::new(RichText::new("✦  Appliquer à tous les appareils").font(bold(14.0))).fill(Color32::from_rgba_unmultiplied(124, 92, 255, 170)))
                .clicked()
            {
                let mut cfg = self.shared.config.lock().unwrap();
                for d in devices {
                    let n = d.info.zones.len().max(1);
                    cfg.profile_mut().devices.insert(
                        d.info.id.clone(),
                        Lighting::Software {
                            zones: vec![self.sync_spec.clone(); n],
                        },
                    );
                    changed.push(d.info.id.clone());
                }
            }
            label_dim(ui, "Les appareils sans mode direct reçoivent l'effet intégré le plus proche.");
        });
    }

    fn settings(&mut self, ui: &mut Ui, reload: &mut bool) {
        let mut cfg = self.shared.config.lock().unwrap();
        let mut save = false;
        let mut autostart: Option<bool> = None;
        card(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.label(RichText::new("Réglages").font(bold(20.0)));
            ui.add_space(6.0);
            section(ui, "Démarrage et fermeture");
            ui.horizontal(|ui| {
                if toggle(ui, &mut cfg.autostart, "Lancer rJb au démarrage du PC (réduit dans la zone de notification)") {
                    autostart = Some(cfg.autostart);
                    save = true;
                }
            });
            ui.horizontal(|ui| {
                ui.label("Bouton fermer de la fenêtre :");
                if chip(ui, cfg.close_to_tray, "Réduire dans la zone de notification").clicked() {
                    cfg.close_to_tray = true;
                    save = true;
                }
                if chip(ui, !cfg.close_to_tray, "Quitter rJb").clicked() {
                    cfg.close_to_tray = false;
                    save = true;
                }
            });
            label_dim(ui, "Réduit, rJb garde vos effets actifs en n'utilisant presque aucune ressource.");
            section(ui, "Économie");
            save |= toggle(ui, &mut cfg.off_on_sleep, "Éteindre l'éclairage pendant la mise en veille");
            save |= toggle(ui, &mut cfg.off_on_shutdown, "Éteindre l'éclairage en quittant rJb / à l'arrêt du PC");
            ui.horizontal(|ui| {
                ui.label("Images par seconde des effets");
                for f in [15u32, 30, 60, 120] {
                    if chip(ui, cfg.fps == f, &f.to_string()).clicked() {
                        cfg.fps = f;
                        *reload = true;
                    }
                }
            });
            label_dim(ui, "30 i/s suffit pour des effets fluides ; un effet fixe ne consomme rien.");
            section(ui, "Composants internes");
            if toggle(ui, &mut cfg.internal_devices, "Piloter la RAM, la carte mère et la carte graphique") {
                save = true;
            }
            if cfg.internal_devices && !crate::drivers::pawnio::installed() {
                ui.horizontal(|ui| {
                    ui.label(RichText::new("⚠ PawnIO n'est pas installé.").color(Color32::from_rgb(255, 196, 90)));
                    if ui.button("⬇  Installer PawnIO").clicked() {
                        install_pawnio();
                    }
                });
            }
            label_dim(ui, "Utilise le pilote signé PawnIO (le même qu'OpenRGB) pour accéder au bus SMBus.");
        });
        ui.add_space(10.0);
        card(ui, |ui| {
            ui.set_width(ui.available_width());
            section(ui, "Profils");
            let names: Vec<String> = cfg.profiles.keys().cloned().collect();
            for n in names {
                ui.horizontal(|ui| {
                    let active = cfg.active_profile == n;
                    if chip(ui, active, &n).clicked() && !active {
                        cfg.active_profile = n.clone();
                        *reload = true;
                    }
                    if cfg.profiles.len() > 1 && ui.small_button("Supprimer").clicked() {
                        cfg.profiles.remove(&n);
                        if cfg.active_profile == n {
                            cfg.active_profile = cfg.profiles.keys().next().unwrap().clone();
                        }
                        *reload = true;
                    }
                });
            }
            ui.horizontal(|ui| {
                ui.add(egui::TextEdit::singleline(&mut self.new_profile).hint_text("Nom du nouveau profil").desired_width(220.0));
                let name = self.new_profile.trim().to_string();
                if ui.add_enabled(!name.is_empty() && !cfg.profiles.contains_key(&name), egui::Button::new("＋ Créer à partir de l'actuel")).clicked() {
                    let p = cfg.profile().clone();
                    cfg.profiles.insert(name.clone(), p);
                    cfg.active_profile = name;
                    self.new_profile.clear();
                    *reload = true;
                }
            });
        });
        ui.add_space(10.0);
        card(ui, |ui| {
            ui.set_width(ui.available_width());
            section(ui, "Sauvegarde de la configuration");
            label_dim(ui, "Profils, effets et réglages de tous les appareils, dans un fichier à garder ou à copier sur un autre PC.");
            ui.horizontal(|ui| {
                if ui.button("⬆  Exporter…").clicked() {
                    if let Some(p) = crate::sys::pick_json(true, "rJb-configuration.json") {
                        self.status = Some(match cfg.export_to(&p) {
                            Ok(()) => (true, format!("Configuration exportée dans {}", p.display())),
                            Err(e) => (false, e),
                        });
                    }
                }
                if ui.button("⬇  Importer…").clicked() {
                    if let Some(p) = crate::sys::pick_json(false, "") {
                        self.status = Some(match crate::config::Config::import_from(&p, &cfg) {
                            Ok(c) => {
                                *cfg = c;
                                *reload = true;
                                (true, "Configuration importée et appliquée.".to_string())
                            }
                            Err(e) => (false, e),
                        });
                    }
                }
                let mut logging = crate::log_path().exists();
                if toggle(ui, &mut logging, "Journal de diagnostic") {
                    let p = crate::log_path();
                    if logging {
                        let _ = std::fs::create_dir_all(p.parent().unwrap_or(&p));
                        let _ = std::fs::write(&p, b"");
                        self.status = Some((true, "Journal activé : il sera rempli au prochain lancement de rJb.".into()));
                    } else {
                        let _ = std::fs::remove_file(&p);
                        self.status = None;
                    }
                }
                if ui.button("📁  Ouvrir le dossier").clicked() {
                    if let Some(d) = crate::config::Config::path().parent() {
                        let _ = std::fs::create_dir_all(d);
                        crate::sys::shell_open(&d.to_string_lossy());
                    }
                }
            });
            if let Some((ok, msg)) = &self.status {
                let col = if *ok { GREEN } else { Color32::from_rgb(255, 120, 120) };
                ui.label(RichText::new(msg).color(col).size(12.5));
            }
        });
        ui.add_space(10.0);
        card(ui, |ui| {
            ui.set_width(ui.available_width());
            section(ui, "Appareils ignorés");
            let devices = self.shared.devices.read().unwrap().clone();
            for d in &devices {
                let mut on = !cfg.disabled.contains(&d.info.id);
                if toggle(ui, &mut on, &d.info.name) {
                    if on {
                        cfg.disabled.remove(&d.info.id);
                    } else {
                        cfg.disabled.insert(d.info.id.clone());
                    }
                    *reload = true;
                }
            }
        });
        ui.add_space(10.0);
        card(ui, |ui| {
            ui.set_width(ui.available_width());
            section(ui, "À propos");
            ui.label(format!("rJb {} — logiciel libre (GPL-2.0-or-later), créé par jeremdevgit", env!("CARGO_PKG_VERSION")));
            label_dim(ui, "Protocoles des appareils issus du travail de la communauté OpenRGB (GPL-2.0-or-later).");
            label_dim(ui, "Modules du pilote PawnIO inclus : licence LGPL-2.1, sources sur github.com/namazso/PawnIO.Modules.");
            label_dim(ui, &format!("Configuration : {}", crate::config::Config::path().display()));
            ui.horizontal(|ui| {
                ui.label(RichText::new("●").color(PINK));
                ui.label(RichText::new("●").color(GREEN));
                ui.label(RichText::new("●").color(CYAN));
            });
        });
        ui.add_space(10.0);
        card(ui, |ui| {
            ui.set_width(ui.available_width());
            section(ui, "Mes autres logiciels");
            label_dim(ui, "Libres, gratuits et sans publicité. Chaque bouton ouvre la page de téléchargement officielle sur GitHub.");
            ui.add_space(4.0);
            for (name, desc, url) in MES_LOGICIELS {
                ui.horizontal(|ui| {
                    if ui.button("Télécharger").clicked() {
                        crate::sys::open_as_user(url);
                    }
                    ui.label(RichText::new(*name).strong());
                    label_dim(ui, desc);
                });
            }
            ui.add_space(4.0);
            if ui.button("Tous mes logiciels sur GitHub").clicked() {
                crate::sys::open_as_user("https://github.com/jeremdevgit");
            }
        });
        if *reload || save {
            cfg.save();
        }
        drop(cfg);
        // peut afficher une invite UAC : hors du verrou de la configuration
        if let Some(on) = autostart {
            crate::install::set_autostart(on);
            let real = crate::install::autostart_enabled();
            let mut cfg = self.shared.config.lock().unwrap();
            if cfg.autostart != real {
                cfg.autostart = real;
                cfg.save();
            }
        }
    }

    fn help(&mut self, ui: &mut Ui) {
        let conflicts = self.conflicts.get_or_insert_with(detect_conflicts).clone();
        card(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.label(RichText::new("Aide").font(bold(20.0)));
            ui.add_space(4.0);
            section(ui, "Vérification rapide");
            let warn = Color32::from_rgb(255, 196, 90);
            let line = |ui: &mut Ui, ok: bool, text: &str| {
                ui.horizontal(|ui| {
                    ui.label(RichText::new(if ok { "✔" } else { "⚠" }).color(if ok { GREEN } else { warn }));
                    ui.label(text);
                });
            };
            if conflicts.is_empty() {
                line(ui, true, "Aucun autre logiciel RGB détecté en cours d'exécution.");
            } else {
                line(ui, false, &format!("Logiciels RGB ouverts qui peuvent reprendre la main : {}", conflicts.join(", ")));
            }
            line(
                ui,
                crate::is_elevated(),
                if crate::is_elevated() {
                    "rJb a les droits administrateur (RAM et carte mère accessibles)."
                } else {
                    "rJb n'a pas les droits administrateur : RAM et carte mère indisponibles."
                },
            );
            let pawn = crate::drivers::pawnio::installed();
            line(ui, pawn, if pawn { "Le pilote PawnIO est installé." } else { "Le pilote PawnIO n'est pas installé." });
            ui.horizontal(|ui| {
                if ui.button("⟳  Revérifier").clicked() {
                    self.conflicts = None;
                }
                if !crate::is_elevated() && ui.button("🛡  Redémarrer en administrateur").clicked() {
                    crate::relaunch_elevated();
                }
                if !pawn && ui.button("⬇  Installer PawnIO").clicked() {
                    install_pawnio();
                }
                if ui.button("⟳  Rechercher les appareils").clicked() {
                    self.shared.send(Cmd::Rescan);
                }
            });
        });
        ui.add_space(10.0);
        card(ui, |ui| {
            ui.set_width(ui.available_width());
            section(ui, "Problèmes fréquents");
            for (i, (q, a)) in FAQ.iter().enumerate() {
                egui::CollapsingHeader::new(RichText::new(*q).font(bold(14.5)))
                    .id_salt(("faq", i))
                    .default_open(i == 0)
                    .show(ui, |ui| {
                        for l in a.lines() {
                            ui.label(RichText::new(l).color(DIM));
                        }
                    });
            }
        });
    }
}

/// Logiciels qui pilotent eux aussi l'éclairage (nom du processus, nom affiché).
const RGB_SOFTWARE: &[(&str, &str)] = &[
    ("lghub.exe", "Logitech G HUB"),
    ("lghub_agent.exe", "Logitech G HUB"),
    ("lghub_system_tray.exe", "Logitech G HUB"),
    ("lcore.exe", "Logitech Gaming Software"),
    ("icue.exe", "Corsair iCUE"),
    ("razer synapse 3.exe", "Razer Synapse"),
    ("razer synapse service process.exe", "Razer Synapse"),
    ("razercentralservice.exe", "Razer Synapse"),
    ("armourycrate.exe", "ASUS Armoury Crate"),
    ("lightingservice.exe", "ASUS Aura"),
    ("msi center.exe", "MSI Center"),
    ("msi.centralserver.exe", "MSI Center"),
    ("mysticlight.exe", "MSI Mystic Light"),
    ("rgbfusion.exe", "Gigabyte RGB Fusion"),
    ("gcc.exe", "Gigabyte Control Center"),
    ("steelseriesgg.exe", "SteelSeries GG"),
    ("steelseriesengine.exe", "SteelSeries GG"),
    ("nzxt cam.exe", "NZXT CAM"),
    ("l-connect 3.exe", "Lian Li L-Connect"),
    ("signalrgb.exe", "SignalRGB"),
    ("signalrgblauncher.exe", "SignalRGB"),
    ("openrgb.exe", "OpenRGB"),
    ("ngenuity2.exe", "HyperX NGENUITY"),
    ("wootility.exe", "Wootility"),
    ("polychromergb.exe", "ASRock Polychrome"),
    ("polychrome rgb.exe", "ASRock Polychrome"),
    ("aurasync.exe", "ASUS Aura Sync"),
    ("thermaltake.exe", "Thermaltake TT RGB Plus"),
    ("masterplus.exe", "Cooler Master MasterPlus"),
];

fn detect_conflicts() -> Vec<&'static str> {
    let running = crate::sys::running_processes();
    let mut out: Vec<&'static str> = Vec::new();
    for (exe, name) in RGB_SOFTWARE {
        if running.iter().any(|r| r == exe) && !out.contains(name) {
            out.push(name);
        }
    }
    out
}

const FAQ: &[(&str, &str)] = &[
    (
        "Un appareil n'apparaît pas",
        "• Fermez le logiciel du fabricant (G HUB, iCUE, Synapse, Armoury Crate, MSI Center, SignalRGB…) : \
deux logiciels ne peuvent pas piloter le même appareil. Pensez à le quitter aussi depuis la zone de notification.\n\
• Cliquez sur « Rechercher les appareils » après avoir branché ou réveillé l'appareil.\n\
• Branchez-le directement sur le PC plutôt que sur un hub ou un écran ; certains modèles sans fil ne se pilotent qu'en filaire.\n\
• Vérifiez qu'il n'est pas dans « Appareils ignorés » (Réglages).",
    ),
    (
        "La RAM, la carte mère ou la carte graphique ne sont pas détectées",
        "• rJb doit être lancé en administrateur (automatique s'il démarre avec Windows depuis l'installation).\n\
• Le pilote PawnIO doit être installé (bouton ci-dessus) et « Piloter la RAM, la carte mère… » activé dans les Réglages.\n\
• Certains anti-triche (Riot Vanguard, FACEIT…) bloquent l'accès au bus des composants : quittez-les ou redémarrez sans eux.\n\
• Les logiciels de surveillance (HWiNFO, AIDA64…) partagent le bus : rJb attend son tour, mais un logiciel RGB constructeur peut bloquer.\n\
• Sur les cartes mères récentes, l'éclairage passe souvent par un contrôleur USB : il apparaît alors comme un appareil USB.",
    ),
    (
        "Les couleurs changent toutes seules ou reviennent à l'effet d'origine",
        "• Un autre logiciel RGB tourne encore en arrière-plan (service ou icône de la zone de notification).\n\
• Windows 11 : Paramètres › Personnalisation › Éclairage dynamique. Désactivez « Utiliser l'éclairage dynamique sur mes appareils » \
ou retirez-lui la priorité, sinon Windows reprend la main sur les claviers et souris compatibles.\n\
• Logitech : désactivez le « mode mémoire intégrée » dans G HUB avant de le désinstaller ou de le quitter.",
    ),
    (
        "Mes ventilateurs ou bandes LED n'ont pas le bon nombre de LED",
        "• Sur la page de l'appareil, réglez la taille de la zone (connecteurs ARGB, Commander, Lighting Node, Uni Hub…).\n\
• Les contrôleurs ne savent pas toujours ce qui est branché : indiquez le total des LED en série sur le connecteur.",
    ),
    (
        "rJb ne démarre pas avec Windows",
        "• Activez « Lancer rJb au démarrage du PC » dans les Réglages.\n\
• Vérifiez qu'il n'est pas désactivé dans le Gestionnaire des tâches › Applications de démarrage.\n\
• Installé dans Program Files, rJb démarre directement en administrateur sans invite.",
    ),
    (
        "L'éclairage s'éteint en veille ou à l'arrêt",
        "• C'est le comportement des options « Économie » dans les Réglages : désactivez-les pour garder la lumière.\n\
• Certaines cartes mères coupent l'alimentation USB à l'arrêt : réglage « ErP » / « Deep Sleep » dans le BIOS.",
    ),
    (
        "rJb utilise trop de ressources",
        "• Un effet fixe ne consomme rien ; les effets animés coûtent selon les images par seconde (30 suffit).\n\
• Fermez la fenêtre : rJb continue dans la zone de notification avec quelques Mo de mémoire.",
    ),
    (
        "Transférer mes réglages sur un autre PC",
        "• Réglages › Sauvegarde de la configuration › Exporter, puis Importer sur l'autre PC.\n\
• Les appareils sont reconnus par leur modèle et leur port : sur un autre PC, certains devront être réglés à nouveau.",
    ),
    (
        "Signaler un problème",
        "• Réglages › Sauvegarde : activez « Journal de diagnostic », relancez rJb et reproduisez le problème.\n\
• Le fichier log.txt (Réglages › Ouvrir le dossier) est à joindre à votre signalement, avec le modèle exact de l'appareil.",
    ),
];

/// Installe le pilote PawnIO via winget (fenêtre visible, l'utilisateur valide l'UAC).
fn install_pawnio() {
    let _ = std::process::Command::new(crate::sys::system_tool("cmd.exe"))
        .args([
            "/C",
            "winget install --id namazso.PawnIO -e --accept-package-agreements --accept-source-agreements & echo. & echo Relancez rJb une fois l'installation terminee. & pause",
        ])
        .spawn();
}

/// Complète la liste des effets pour qu'il y en ait un par zone.
fn fill_zones(zones: &mut Vec<EffectSpec>, info: &DeviceInfo) {
    let n = info.zones.len().max(1);
    if zones.is_empty() {
        zones.push(EffectSpec::default());
    }
    while zones.len() < n {
        zones.push(zones[0].clone());
    }
    zones.truncate(n);
}

/// Logiciels de l'auteur (jeremdevgit) : nom, description, page de téléchargement.
const MES_LOGICIELS: &[(&str, &str, &str)] = &[
    ("Boîte à outils", "Windows et macOS · entretien, nettoyage et réglages cachés", "https://github.com/jeremdevgit/boite-a-outils/releases/latest"),
    ("Pomme", "Windows · lecteur Apple Music et musique locale", "https://github.com/jeremdevgit/pomme/releases/latest"),
    ("Ludothèque", "macOS · tous vos jeux dans un seul lanceur", "https://github.com/jeremdevgit/ludotheque/releases/latest"),
    ("Suite Libre", "navigateur · six outils sans compte", "https://jeremdevgit.github.io/suite-libre/"),
    ("Batailles de France", "navigateur · jeu de stratégie en temps réel", "https://github.com/jeremdevgit/batailles-de-france/releases/latest"),
];
