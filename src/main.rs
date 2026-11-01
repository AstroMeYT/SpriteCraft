#![cfg_attr(all(target_os = "windows", not(debug_assertions)), windows_subsystem = "windows")]
mod app;
mod config;
mod dialogs;
mod doc;
mod effects;
mod export;
mod features;
mod palettes;
mod start;
mod text;
mod ui;

use doc::*;
use eframe::egui::{self, pos2, vec2, Align2, Color32, FontId, Key, Pos2, Rect, Sense, Stroke, TextureHandle, TextureOptions, Ui};
use std::path::{Path, PathBuf};
use std::sync::Arc;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Tool {
    Pencil,
    Eraser,
    Bucket,
    Picker,
    Line,
    Rect,
    Ellipse,
    Select,
    Lasso,
    Wand,
    Move,
    Shade,
    Spray,
    Text,
    Hand,
}

const TOOLS: &[(Tool, &str, &str)] = &[
    (Tool::Pencil, "Pencil", "B"),
    (Tool::Eraser, "Eraser", "E"),
    (Tool::Bucket, "Paint Bucket", "G"),
    (Tool::Picker, "Eyedropper", "I"),
    (Tool::Line, "Line", "L"),
    (Tool::Rect, "Rectangle", "R"),
    (Tool::Ellipse, "Ellipse", "O"),
    (Tool::Select, "Rectangular Marquee", "M"),
    (Tool::Lasso, "Lasso (free-form selection)", "Q"),
    (Tool::Wand, "Magic Wand", "W"),
    (Tool::Move, "Move", "V"),
    (Tool::Shade, "Lighten / Darken", "D"),
    (Tool::Spray, "Spray", "A"),
    (Tool::Text, "Text (pixelated)", "T"),
    (Tool::Hand, "Hand (or hold Space)", "H"),
];

struct Stroke_ {
    tool: Tool,
    button: u8,
    start: (i32, i32),
    last: (i32, i32),
    base: Vec<Px>,
    pts: Vec<(i32, i32)>,
    lifted: Vec<(i32, i32, Px)>,
    mask0: Vec<bool>,
    sel_base: Option<Vec<bool>>,
}

struct TabState {
    doc: Doc,
    undo: Vec<Doc>,
    redo: Vec<Doc>,
    cur_frame: usize,
    cur_layer: usize,
    selection: Option<Vec<bool>>,
    path: Option<PathBuf>,
    modified: bool,
    zoom: f32,
    pan: egui::Vec2,
    need_fit: bool,
    uid: usize,
}

impl TabState {
    fn empty() -> TabState {
        TabState { doc: Doc::new(1, 1), undo: vec![], redo: vec![], cur_frame: 0, cur_layer: 0, selection: None, path: None, modified: false, zoom: 8.0, pan: vec2(0.0, 0.0), need_fit: true, uid: 0 }
    }
}

struct TextCache {
    key: String,
    w: usize,
    h: usize,
    px: Vec<Px>,
}

struct Clip {
    w: usize,
    h: usize,
    px: Vec<Px>,
}

struct FileBrowser {
    dir: PathBuf,
    name: String,
    exts: Vec<&'static str>,
}

struct ExportState {
    dir: String,
    name: String,
    kind: usize,
    scale: u32,
    cols: usize,
    json: bool,
}

enum Dialog {
    None,
    New { w: usize, h: usize },
    Open(FileBrowser),
    Save(FileBrowser),
    Import { fb: FileBrowser, resize: bool },
    Export(ExportState),
    Resize { w: usize, h: usize, anchor: (i32, i32), scale: bool },
    Adjust { hue: f32, sat: f32, val: f32, all: bool },
    Shortcuts,
    About,
    ConfirmClose(usize),
    Effects { tab: usize, thick: usize, diag: bool, inner: bool, sdx: i32, sdy: i32, light: (i32, i32), depth: usize, strength: f32, hue: f32, all: bool },
    RotSprite { angle: f32, all: bool },
    Replace { tol: i32, scope: usize },
    Tag { idx: Option<usize>, name: String, from: usize, to: usize, dir: usize, color: [u8; 3] },
    RenameFrame(String),
    Recover(Vec<config::Crashed>),
    ConfirmQuit,
}

#[derive(Clone, Copy)]
struct Theme {
    light: bool,
    title: Color32,
    workspace: Color32,
    well: Color32,
    tab: Color32,
    tab_hover: Color32,
    tab_sel: Color32,
    cell: Color32,
    ruler: Color32,
    border: Color32,
    btn_hover: Color32,
    text: Color32,
    strong: Color32,
    dim: Color32,
    card: Color32,
}

impl Theme {
    fn new(light: bool) -> Theme {
        if light {
            Theme {
                light,
                title: Color32::from_rgb(236, 236, 238),
                workspace: Color32::from_rgb(214, 214, 218),
                well: Color32::from_rgb(200, 200, 205),
                tab: Color32::from_rgb(222, 222, 226),
                tab_hover: Color32::from_rgb(232, 232, 236),
                tab_sel: Color32::from_rgb(248, 248, 250),
                cell: Color32::from_rgb(226, 226, 230),
                ruler: Color32::from_rgb(228, 228, 232),
                border: Color32::from_rgb(150, 150, 158),
                btn_hover: Color32::from_rgb(205, 205, 212),
                text: Color32::from_gray(50),
                strong: Color32::from_gray(15),
                dim: Color32::from_gray(100),
                card: Color32::from_rgb(244, 244, 247),
            }
        } else {
            Theme {
                light,
                title: Color32::from_rgb(31, 31, 33),
                workspace: Color32::from_rgb(34, 34, 36),
                well: Color32::from_rgb(26, 26, 28),
                tab: Color32::from_rgb(36, 36, 38),
                tab_hover: Color32::from_rgb(44, 44, 47),
                tab_sel: Color32::from_rgb(52, 52, 56),
                cell: Color32::from_rgb(50, 50, 54),
                ruler: Color32::from_rgb(46, 46, 50),
                border: Color32::from_rgb(86, 86, 92),
                btn_hover: Color32::from_rgb(70, 70, 76),
                text: Color32::from_gray(205),
                strong: Color32::WHITE,
                dim: Color32::from_gray(150),
                card: Color32::from_rgb(46, 46, 50),
            }
        }
    }
}

struct App {
    doc: Doc,
    undo: Vec<Doc>,
    redo: Vec<Doc>,
    cur_frame: usize,
    cur_layer: usize,
    tool: Tool,
    color: Color32,
    color2: Color32,
    hex_text: String,
    brush: i32,
    round: bool,
    dither: bool,
    pixel_perfect: bool,
    sym_x: bool,
    sym_y: bool,
    fill_shapes: bool,
    global_fill: bool,
    tolerance: i32,
    shade_darken: bool,
    shade_strength: f32,
    spray_density: f32,
    zoom: f32,
    pan: egui::Vec2,
    need_fit: bool,
    grid: bool,
    rulers: bool,
    onion: bool,
    onion_n: usize,
    playing: bool,
    play_frame: usize,
    play_last: f64,
    tile_preview: bool,
    palette: Vec<Color32>,
    palette_remove: bool,
    selection: Option<Vec<bool>>,
    clipboard: Option<Clip>,
    stroke: Option<Stroke_>,
    panning: bool,
    shift: bool,
    hover: Option<(i32, i32)>,
    main_tex: Option<TextureHandle>,
    prev_tex: Option<TextureHandle>,
    thumbs: Vec<Option<TextureHandle>>,
    dirty: bool,
    dirty_all: bool,
    dialog: Dialog,
    status: String,
    path: Option<PathBuf>,
    modified: bool,
    rename_layer: Option<usize>,
    rng: u64,
    tabs: Vec<TabState>,
    active: usize,
    uid: usize,
    uid_counter: usize,
    logo_tex: Option<TextureHandle>,
    text: String,
    text_font_name: String,
    fonts: Vec<(String, Option<PathBuf>)>,
    text_size: f32,
    text_smooth: bool,
    text_thresh: f32,
    text_cache: Option<TextCache>,
    text_font: Option<(String, ab_glyph::FontVec)>,
    text_tex: Option<(String, TextureHandle)>,
    th: Theme,
    config: config::Config,
    show_home: bool,
    new_w: usize,
    new_h: usize,
    project_colors: Vec<(Px, usize)>,
    colors_dirty: bool,
    play_tag: Option<usize>,
    play_pos: usize,
    session: String,
    last_autosave: f64,
    palette_open: bool,
    palette_query: String,
    palette_sel: usize,
    quit_ok: bool,
}

pub fn logo_rgba() -> Vec<u8> {
    let n = 64usize;
    let mut buf = vec![0u8; n * n * 4];
    let r = 12.0f32;
    for y in 0..n {
        for x in 0..n {
            let (fx, fy) = (x as f32 + 0.5, y as f32 + 0.5);
            let cx = fx.clamp(r, n as f32 - r);
            let cy = fy.clamp(r, n as f32 - r);
            let inside = (fx - cx).powi(2) + (fy - cy).powi(2) <= r * r;
            if inside {
                let i = (y * n + x) * 4;
                buf[i..i + 4].copy_from_slice(&[35, 35, 38, 255]);
            }
        }
    }
    let sq: [(usize, usize, u32); 8] = [(12, 12, 0xef7d57), (20, 20, 0xffcd75), (28, 28, 0xa7f070), (36, 36, 0x41a6f6), (44, 44, 0xb13e53), (28, 12, 0x73eff7), (12, 44, 0x5d275d), (44, 12, 0x3b5dc9)];
    for (sx, sy, c) in sq {
        for y in sy..sy + 8 {
            for x in sx..sx + 8 {
                let i = (y * n + x) * 4;
                buf[i..i + 4].copy_from_slice(&[(c >> 16) as u8, (c >> 8) as u8, c as u8, 255]);
            }
        }
    }
    buf
}

fn home_dir() -> PathBuf {
    std::env::var_os("HOME").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("."))
}

fn projects_dir() -> PathBuf {
    let d = home_dir().join("SpriteCraft");
    let _ = std::fs::create_dir_all(&d);
    d
}

fn px_of(c: Color32) -> Px {
    c.to_srgba_unmultiplied()
}

fn color_of(p: Px) -> Color32 {
    Color32::from_rgba_unmultiplied(p[0], p[1], p[2], p[3])
}

fn hex_color(h: u32) -> Color32 {
    Color32::from_rgb((h >> 16) as u8, (h >> 8) as u8, h as u8)
}

fn apply_theme(ctx: &egui::Context, light: bool) {
    if light {
        let mut v = egui::Visuals::light();
        v.panel_fill = Color32::from_rgb(238, 238, 241);
        v.window_fill = Color32::from_rgb(246, 246, 249);
        v.extreme_bg_color = Color32::from_rgb(252, 252, 253);
        v.faint_bg_color = Color32::from_rgb(230, 230, 234);
        v.widgets.inactive.weak_bg_fill = Color32::from_rgb(222, 222, 228);
        v.widgets.inactive.bg_fill = Color32::from_rgb(222, 222, 228);
        v.widgets.hovered.weak_bg_fill = Color32::from_rgb(206, 206, 214);
        v.widgets.hovered.bg_fill = Color32::from_rgb(206, 206, 214);
        v.widgets.active.weak_bg_fill = Color32::from_rgb(38, 120, 220);
        v.widgets.active.bg_fill = Color32::from_rgb(38, 120, 220);
        v.selection.bg_fill = Color32::from_rgb(38, 120, 220);
        v.selection.stroke = Stroke::new(1.0, Color32::WHITE);
        let r = egui::Rounding::same(4.0);
        v.widgets.noninteractive.rounding = r;
        v.widgets.inactive.rounding = r;
        v.widgets.hovered.rounding = r;
        v.widgets.active.rounding = r;
        v.widgets.open.rounding = r;
        v.window_rounding = egui::Rounding::same(8.0);
        v.menu_rounding = egui::Rounding::same(6.0);
        ctx.set_visuals(v);
        ctx.style_mut(|s| {
            s.spacing.item_spacing = vec2(6.0, 4.0);
            s.spacing.button_padding = vec2(8.0, 3.0);
        });
        return;
    }
    let mut v = egui::Visuals::dark();
    v.panel_fill = Color32::from_rgb(40, 40, 42);
    v.window_fill = Color32::from_rgb(48, 48, 50);
    v.extreme_bg_color = Color32::from_rgb(28, 28, 30);
    v.faint_bg_color = Color32::from_rgb(46, 46, 48);
    v.window_stroke = Stroke::new(1.0, Color32::from_rgb(70, 70, 74));
    v.widgets.noninteractive.bg_stroke = Stroke::new(1.0, Color32::from_rgb(58, 58, 62));
    v.widgets.inactive.weak_bg_fill = Color32::from_rgb(62, 62, 66);
    v.widgets.inactive.bg_fill = Color32::from_rgb(62, 62, 66);
    v.widgets.hovered.weak_bg_fill = Color32::from_rgb(78, 78, 84);
    v.widgets.hovered.bg_fill = Color32::from_rgb(78, 78, 84);
    v.widgets.active.weak_bg_fill = Color32::from_rgb(38, 120, 220);
    v.widgets.active.bg_fill = Color32::from_rgb(38, 120, 220);
    v.selection.bg_fill = Color32::from_rgb(38, 120, 220);
    v.selection.stroke = Stroke::new(1.0, Color32::WHITE);
    let r = egui::Rounding::same(4.0);
    v.widgets.noninteractive.rounding = r;
    v.widgets.inactive.rounding = r;
    v.widgets.hovered.rounding = r;
    v.widgets.active.rounding = r;
    v.widgets.open.rounding = r;
    v.window_rounding = egui::Rounding::same(8.0);
    v.menu_rounding = egui::Rounding::same(6.0);
    ctx.set_visuals(v);
    ctx.style_mut(|s| {
        s.spacing.item_spacing = vec2(6.0, 4.0);
        s.spacing.button_padding = vec2(8.0, 3.0);
    });
}

fn draw_icon(p: &egui::Painter, r: Rect, tool: Tool, col: Color32) {
    let c = r.center();
    let u = r.width() / 34.0;
    let st = Stroke::new(2.0 * u.max(0.8), col);
    let thin = Stroke::new(1.3 * u.max(0.8), col);
    let pt = |x: f32, y: f32| c + vec2(x, y) * u;
    match tool {
        Tool::Pencil => {
            p.line_segment([pt(-8.0, 8.0), pt(6.0, -6.0)], Stroke::new(4.5 * u, col));
            p.add(egui::Shape::convex_polygon(vec![pt(-11.0, 11.0), pt(-9.0, 5.0), pt(-5.0, 9.0)], col, Stroke::NONE));
            p.line_segment([pt(8.0, -8.0), pt(10.0, -10.0)], Stroke::new(4.5 * u, col));
        }
        Tool::Eraser => {
            p.add(egui::Shape::convex_polygon(vec![pt(-10.0, 2.0), pt(2.0, -10.0), pt(10.0, -2.0), pt(-2.0, 10.0)], Color32::TRANSPARENT, st));
            p.line_segment([pt(-5.0, -3.0), pt(3.0, 5.0)], thin);
        }
        Tool::Bucket => {
            p.add(egui::Shape::convex_polygon(vec![pt(-9.0, 0.0), pt(0.0, -9.0), pt(8.0, -1.0), pt(-1.0, 8.0)], Color32::TRANSPARENT, st));
            p.circle_filled(pt(10.0, 7.0), 2.8 * u, col);
        }
        Tool::Picker => {
            p.line_segment([pt(-9.0, 9.0), pt(3.0, -3.0)], st);
            p.circle_stroke(pt(6.0, -6.0), 4.5 * u, st);
        }
        Tool::Line => {
            p.line_segment([pt(-9.0, 9.0), pt(9.0, -9.0)], st);
            p.rect_filled(Rect::from_center_size(pt(-9.0, 9.0), vec2(5.0, 5.0) * u), 0.0, col);
            p.rect_filled(Rect::from_center_size(pt(9.0, -9.0), vec2(5.0, 5.0) * u), 0.0, col);
        }
        Tool::Rect => {
            p.rect_stroke(Rect::from_center_size(c, vec2(18.0, 14.0) * u), 1.0, st);
        }
        Tool::Ellipse => {
            let pts: Vec<Pos2> = (0..32).map(|i| { let a = i as f32 / 32.0 * std::f32::consts::TAU; pt(a.cos() * 10.0, a.sin() * 7.5) }).collect();
            p.add(egui::Shape::closed_line(pts, st));
        }
        Tool::Select => {
            let (a, b) = (pt(-9.0, -8.0), pt(9.0, 8.0));
            for (x0, x1) in [(a.x, b.x)] {
                for y in [a.y, b.y] {
                    p.line_segment([pos2(x0, y), pos2(x0 + 5.0 * u, y)], thin);
                    p.line_segment([pos2(x1 - 5.0 * u, y), pos2(x1, y)], thin);
                }
                for x in [x0, x1] {
                    p.line_segment([pos2(x, a.y), pos2(x, a.y + 4.0 * u)], thin);
                    p.line_segment([pos2(x, b.y - 4.0 * u), pos2(x, b.y)], thin);
                }
            }
        }
        Tool::Lasso => {
            let pts: Vec<Pos2> = (0..28).map(|i| { let a = i as f32 / 28.0 * std::f32::consts::TAU; pt(a.cos() * 9.0, a.sin() * 6.0 - 2.0) }).collect();
            p.add(egui::Shape::dashed_line(&{ let mut v = pts.clone(); v.push(pts[0]); v }, thin, 3.0 * u, 2.0 * u));
            p.line_segment([pt(-6.0, 4.0), pt(-9.0, 10.0)], thin);
            p.circle_stroke(pt(-9.5, 11.0), 1.8 * u, thin);
        }
        Tool::Wand => {
            p.line_segment([pt(-10.0, 10.0), pt(2.0, -2.0)], st);
            for (dx, dy) in [(6.0, -6.0)] {
                p.line_segment([pt(dx - 5.0, dy), pt(dx + 5.0, dy)], thin);
                p.line_segment([pt(dx, dy - 5.0), pt(dx, dy + 5.0)], thin);
                p.line_segment([pt(dx - 3.0, dy - 3.0), pt(dx + 3.0, dy + 3.0)], thin);
                p.line_segment([pt(dx - 3.0, dy + 3.0), pt(dx + 3.0, dy - 3.0)], thin);
            }
        }
        Tool::Move => {
            p.line_segment([pt(-10.0, 0.0), pt(10.0, 0.0)], thin);
            p.line_segment([pt(0.0, -10.0), pt(0.0, 10.0)], thin);
            for (d, e) in [((1.0, 0.0), (0.0, 1.0)), ((-1.0, 0.0), (0.0, 1.0)), ((0.0, 1.0), (1.0, 0.0)), ((0.0, -1.0), (1.0, 0.0))] {
                let tip = pt(d.0 * 11.0, d.1 * 11.0);
                let b1 = pt(d.0 * 6.0 + e.0 * 3.5, d.1 * 6.0 + e.1 * 3.5);
                let b2 = pt(d.0 * 6.0 - e.0 * 3.5, d.1 * 6.0 - e.1 * 3.5);
                p.add(egui::Shape::convex_polygon(vec![tip, b1, b2], col, Stroke::NONE));
            }
        }
        Tool::Shade => {
            p.circle_stroke(c, 9.0 * u, st);
            p.add(egui::Shape::convex_polygon(vec![pt(0.0, -9.0), pt(-6.0, -6.5), pt(-9.0, 0.0), pt(-6.0, 6.5), pt(0.0, 9.0)], col, Stroke::NONE));
        }
        Tool::Spray => {
            for (x, y) in [(-6.0, 6.0), (-2.0, 2.0), (2.0, -2.0), (6.0, -6.0), (-6.0, -2.0), (2.0, 6.0), (6.0, 2.0), (-2.0, -6.0), (0.0, 0.0)] {
                p.circle_filled(pt(x, y), 1.5 * u, col);
            }
        }
        Tool::Text => {
            p.line_segment([pt(-8.0, -8.0), pt(8.0, -8.0)], Stroke::new(3.0 * u, col));
            p.line_segment([pt(0.0, -8.0), pt(0.0, 9.0)], Stroke::new(3.0 * u, col));
            p.line_segment([pt(-4.0, 9.0), pt(4.0, 9.0)], thin);
        }
        Tool::Hand => {
            p.rect_stroke(Rect::from_center_size(pt(0.0, 3.0), vec2(14.0, 11.0) * u), 3.0, st);
            for x in [-4.5, -1.5, 1.5, 4.5] {
                p.line_segment([pt(x, -3.0), pt(x, -10.0 + (x as f32).abs())], st);
            }
        }
    }
}

fn eye_icon(p: &egui::Painter, r: Rect, on: bool, col: Color32) {
    let c = r.center();
    if on {
        let pts: Vec<Pos2> = (0..24).map(|i| { let a = i as f32 / 24.0 * std::f32::consts::TAU; c + vec2(a.cos() * 7.0, a.sin() * 4.0) }).collect();
        p.add(egui::Shape::closed_line(pts, Stroke::new(1.4, col)));
        p.circle_filled(c, 2.0, col);
    } else {
        p.line_segment([c + vec2(-6.0, 0.0), c + vec2(6.0, 0.0)], Stroke::new(1.4, col));
    }
}

fn lock_icon(p: &egui::Painter, r: Rect, on: bool, col: Color32) {
    let c = r.center();
    p.rect_filled(Rect::from_center_size(c + vec2(0.0, 2.0), vec2(8.0, 6.0)), 1.0, col);
    let sh = if on { -2.0 } else { 0.0 };
    p.line_segment([c + vec2(-2.5, -1.0), c + vec2(-2.5, -4.0 + sh)], Stroke::new(1.3, col));
    p.line_segment([c + vec2(2.5, -1.0), c + vec2(2.5, -4.0)], Stroke::new(1.3, col));
    p.line_segment([c + vec2(-2.5, -4.0 + sh), c + vec2(2.5, -4.0 + sh)], Stroke::new(1.3, col));
}

fn browser_ui(ui: &mut Ui, fb: &mut FileBrowser, list: bool) {
    ui.horizontal(|ui| {
        ui.label("Folder");
        let mut d = fb.dir.to_string_lossy().to_string();
        if ui.add(egui::TextEdit::singleline(&mut d).desired_width((ui.available_width() - 70.0).max(120.0))).changed() {
            fb.dir = PathBuf::from(d);
        }
        if ui.button("Up").clicked() {
            if let Some(p) = fb.dir.parent() {
                fb.dir = p.to_path_buf();
            }
        }
    });
    if list {
        let mut entries: Vec<(String, bool)> = std::fs::read_dir(&fb.dir)
            .map(|rd| rd.filter_map(|e| e.ok()).map(|e| (e.file_name().to_string_lossy().to_string(), e.path().is_dir())).collect())
            .unwrap_or_default();
        entries.retain(|(n, d)| !n.starts_with('.') && (*d || fb.exts.iter().any(|x| n.to_lowercase().ends_with(x))));
        entries.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.to_lowercase().cmp(&b.0.to_lowercase())));
        let lh = (ui.available_height() - 70.0).max(100.0);
        egui::ScrollArea::vertical().max_height(lh).min_scrolled_height(lh).auto_shrink([false, false]).show(ui, |ui| {
            for (n, d) in entries {
                let label = if d { format!("[folder]  {n}") } else { n.clone() };
                if ui.selectable_label(!d && fb.name == n, label).clicked() {
                    if d {
                        fb.dir = fb.dir.join(&n);
                    } else {
                        fb.name = n;
                    }
                }
            }
        });
    }
    ui.horizontal(|ui| {
        ui.label("File");
        ui.add(egui::TextEdit::singleline(&mut fb.name).desired_width((ui.available_width() - 4.0).max(120.0)));
    });
}

fn blend_onion(out: &mut [Px], comp: &[Px], tint: [u8; 3], alpha: f32) {
    for (o, s) in out.iter_mut().zip(comp.iter()) {
        if s[3] != 0 {
            *o = over(*o, [tint[0], tint[1], tint[2], s[3]], alpha);
        }
    }
}

fn to_color_image(w: usize, h: usize, px: &[Px]) -> egui::ColorImage {
    let bytes: Vec<u8> = px.iter().flat_map(|p| p.iter().copied()).collect();
    egui::ColorImage::from_rgba_unmultiplied([w, h], &bytes)
}

fn scaled_image(px: &[Px], w: usize, h: usize, s: usize) -> image::RgbaImage {
    let mut img = image::RgbaImage::new((w * s) as u32, (h * s) as u32);
    for y in 0..h * s {
        for x in 0..w * s {
            img.put_pixel(x as u32, y as u32, image::Rgba(px[(y / s) * w + x / s]));
        }
    }
    img
}

fn sanitize_ext(name: &str, ext: &str) -> String {
    if name.to_lowercase().ends_with(&format!(".{ext}")) { name.to_string() } else { format!("{name}.{ext}") }
}

fn path_join(dir: &str, name: &str) -> PathBuf {
    Path::new(dir).join(name)
}

fn main() -> eframe::Result {
    let opts = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1360.0, 860.0])
            .with_min_inner_size([960.0, 620.0])
            .with_title("SpriteCraft")
            .with_decorations(false)
            .with_icon(egui::IconData { rgba: logo_rgba(), width: 64, height: 64 })
            .with_app_id("spritecraft"),
        ..Default::default()
    };
    eframe::run_native("SpriteCraft", opts, Box::new(|cc| Ok(Box::new(App::new(cc)))))
}
