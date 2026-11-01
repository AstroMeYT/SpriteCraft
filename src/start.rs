//! Start page (home screen) and the command palette.
use super::*;
use eframe::egui::{Frame, RichText, ScrollArea};

type Cmd = fn(&mut App, &egui::Context);

pub fn dlg_effects() -> Dialog {
    Dialog::Effects { tab: 0, thick: 1, diag: false, inner: false, sdx: 1, sdy: 1, light: (-1, -1), depth: 2, strength: 0.5, hue: 0.5, all: false }
}

pub fn dlg_new_tag(a: &App) -> Dialog {
    let f = a.cur_frame;
    Dialog::Tag { idx: None, name: format!("Tag {}", a.doc.tags.len() + 1), from: f, to: f, dir: 0, color: [38, 120, 220] }
}

const PRESETS: &[(&str, &str, usize, usize)] = &[
    ("16 x 16", "Icon", 16, 16),
    ("32 x 32", "Tile", 32, 32),
    ("32 x 48", "Character", 32, 48),
    ("64 x 64", "Sprite", 64, 64),
    ("128 x 128", "Scene", 128, 128),
    ("256 x 256", "Large", 256, 256),
    ("128 x 64", "Banner", 128, 64),
    ("64 x 32", "Wide tile", 64, 32),
];

fn ago(p: &std::path::Path) -> Option<String> {
    let secs = std::fs::metadata(p).and_then(|m| m.modified()).ok()?.elapsed().ok()?.as_secs();
    Some(match secs {
        0..=59 => "just now".into(),
        60..=3599 => format!("{} min ago", secs / 60),
        3600..=86399 => format!("{} h ago", secs / 3600),
        _ => format!("{} days ago", secs / 86400),
    })
}

impl App {
    pub fn start_page(&mut self, ctx: &egui::Context) {
        let th = self.th;
        egui::CentralPanel::default().frame(Frame::none().fill(th.workspace)).show(ctx, |ui| {
            if !self.placeholder_only() {
                self.tab_bar(ui);
            }
            ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                let content_w = (ui.available_width() - 60.0).clamp(520.0, 1100.0);
                let pad = ((ui.available_width() - content_w) / 2.0).max(10.0);
                ui.horizontal_top(|ui| {
                    ui.add_space(pad);
                    ui.vertical(|ui| {
                        ui.set_width(content_w);
                        ui.add_space(34.0);
                        ui.horizontal(|ui| {
                            if let Some(t) = &self.logo_tex {
                                ui.image((t.id(), vec2(64.0, 64.0)));
                            }
                            ui.vertical(|ui| {
                                ui.label(RichText::new("SpriteCraft").size(32.0).strong().color(th.strong));
                                ui.label(RichText::new(format!("Pixel art & animation studio  -  version {}", env!("CARGO_PKG_VERSION"))).color(th.dim));
                            });
                        });
                        ui.add_space(26.0);
                        ui.columns(2, |cols| {
                            // ---------------- left: create & open
                            let ui = &mut cols[0];
                            ui.label(RichText::new("Create new").size(18.0).strong().color(th.strong));
                            ui.add_space(6.0);
                            let tile = vec2(138.0, 118.0);
                            ui.spacing_mut().item_spacing = vec2(10.0, 10.0);
                            ui.horizontal_wrapped(|ui| {
                                for &(label, kind, w, h) in PRESETS {
                                    let (rect, resp) = ui.allocate_exact_size(tile, Sense::click());
                                    let p = ui.painter();
                                    p.rect_filled(rect, 8.0, if resp.hovered() { th.tab_hover } else { th.card });
                                    p.rect_stroke(rect, 8.0, Stroke::new(if resp.hovered() { 1.5 } else { 1.0 }, if resp.hovered() { Color32::from_rgb(38, 120, 220) } else { th.border }));
                                    let area = Rect::from_center_size(rect.center() - vec2(0.0, 14.0), vec2(64.0, 56.0));
                                    let s = (area.width() / w as f32).min(area.height() / h as f32);
                                    let r = Rect::from_center_size(area.center(), vec2(w as f32 * s, h as f32 * s));
                                    p.rect_filled(r, 1.0, Color32::from_gray(if th.light { 150 } else { 80 }));
                                    p.rect_stroke(r, 1.0, Stroke::new(1.0, Color32::from_rgb(38, 120, 220)));
                                    p.text(pos2(rect.center().x, rect.bottom() - 30.0), Align2::CENTER_CENTER, label, FontId::proportional(14.0), th.strong);
                                    p.text(pos2(rect.center().x, rect.bottom() - 13.0), Align2::CENTER_CENTER, kind, FontId::proportional(11.5), th.dim);
                                    if resp.clicked() {
                                        self.new_project(w, h);
                                    }
                                }
                            });
                            ui.add_space(12.0);
                            ui.label(RichText::new("Custom size").strong().color(th.strong));
                            ui.horizontal(|ui| {
                                ui.add(egui::DragValue::new(&mut self.new_w).range(1..=1024).suffix(" px"));
                                ui.label("x");
                                ui.add(egui::DragValue::new(&mut self.new_h).range(1..=1024).suffix(" px"));
                                if ui.add(egui::Button::new(RichText::new("Create").strong().color(Color32::WHITE)).fill(Color32::from_rgb(38, 120, 220))).clicked() {
                                    let (w, h) = (self.new_w, self.new_h);
                                    self.new_project(w, h);
                                }
                            });
                            ui.add_space(18.0);
                            ui.label(RichText::new("Open").size(18.0).strong().color(th.strong));
                            ui.horizontal(|ui| {
                                if ui.add_sized([150.0, 34.0], egui::Button::new("Open project...")).clicked() {
                                    self.open_open();
                                }
                                if ui.add_sized([150.0, 34.0], egui::Button::new("Import PNG...")).clicked() {
                                    self.dialog = Dialog::Import { fb: FileBrowser { dir: home_dir(), name: String::new(), exts: vec![".png"] }, resize: true };
                                }
                            });

                            // ---------------- right: recent
                            let ui = &mut cols[1];
                            ui.horizontal(|ui| {
                                ui.label(RichText::new("Recent").size(18.0).strong().color(th.strong));
                                if !self.config.recent.is_empty() && ui.small_button("Clear").clicked() {
                                    self.config.recent.clear();
                                    self.config.save();
                                }
                            });
                            ui.add_space(6.0);
                            if self.config.recent.is_empty() {
                                ui.label(RichText::new("Projects you open or save will appear here.").color(th.dim));
                            }
                            let mut open: Option<PathBuf> = None;
                            for r in self.config.recent.clone() {
                                let path = PathBuf::from(&r);
                                let exists = path.exists();
                                let name = path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or(r.clone());
                                let dir = path.parent().map(|d| d.to_string_lossy().to_string()).unwrap_or_default();
                                let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 50.0), Sense::click());
                                let p = ui.painter();
                                if resp.hovered() {
                                    p.rect_filled(rect, 6.0, th.tab_hover);
                                }
                                let tc = if exists { th.strong } else { th.dim };
                                p.text(rect.min + vec2(12.0, 14.0), Align2::LEFT_CENTER, &name, FontId::proportional(14.5), tc);
                                let sub = if exists { format!("{}   -   {}", ago(&path).unwrap_or_default(), dir) } else { format!("missing   -   {dir}") };
                                let clipped = ui.painter_at(rect.shrink2(vec2(10.0, 0.0)));
                                clipped.text(rect.min + vec2(12.0, 34.0), Align2::LEFT_CENTER, sub, FontId::proportional(11.5), th.dim);
                                if resp.on_hover_text(&r).clicked() && exists {
                                    open = Some(path);
                                }
                            }
                            if let Some(p) = open {
                                self.open_path(&p);
                            }
                        });
                        ui.add_space(30.0);
                        ui.label(RichText::new("Tip: drop .spc or .png files anywhere to open them.   Press Ctrl+K for the command palette.").color(th.dim));
                    });
                });
            });
        });
    }

    // ------------------------------------------------------------ command palette
    pub fn command_palette(&mut self, ctx: &egui::Context) {
        if !self.palette_open {
            return;
        }
        let q = self.palette_query.to_lowercase();
        let terms: Vec<&str> = q.split_whitespace().collect();
        let cmds = commands();
        let mut matches: Vec<usize> = (0..cmds.len()).filter(|&i| terms.iter().all(|t| cmds[i].0.to_lowercase().contains(t))).collect();
        matches.sort_by_key(|&i| !cmds[i].0.to_lowercase().starts_with(terms.first().copied().unwrap_or("")));
        let (up, down, enter, esc) = ctx.input_mut(|i| {
            (
                i.consume_key(egui::Modifiers::NONE, Key::ArrowUp),
                i.consume_key(egui::Modifiers::NONE, Key::ArrowDown),
                i.consume_key(egui::Modifiers::NONE, Key::Enter),
                i.consume_key(egui::Modifiers::NONE, Key::Escape),
            )
        });
        if down {
            self.palette_sel = (self.palette_sel + 1).min(matches.len().saturating_sub(1));
        }
        if up {
            self.palette_sel = self.palette_sel.saturating_sub(1);
        }
        let mut run: Option<usize> = if enter { matches.get(self.palette_sel).copied() } else { None };
        egui::Area::new(egui::Id::new("cmdpal")).order(egui::Order::Foreground).anchor(Align2::CENTER_TOP, [0.0, 90.0]).show(ctx, |ui| {
            Frame::popup(ui.style()).inner_margin(egui::Margin::same(14.0)).rounding(10.0).show(ui, |ui| {
                ui.set_width(580.0);
                let r = ui.add(egui::TextEdit::singleline(&mut self.palette_query).hint_text("Type a command...  (Esc to close)").desired_width(f32::INFINITY).font(egui::TextStyle::Heading));
                r.request_focus();
                if r.changed() {
                    self.palette_sel = 0;
                }
                ui.add_space(6.0);
                ScrollArea::vertical().max_height(380.0).auto_shrink([false, true]).show(ui, |ui| {
                    if matches.is_empty() {
                        ui.label(RichText::new("No matching commands").color(self.th.dim));
                    }
                    for (k, &ci) in matches.iter().enumerate().take(60) {
                        let sel = k == self.palette_sel;
                        let resp = ui.add_sized([ui.available_width(), 28.0], egui::SelectableLabel::new(sel, cmds[ci].0));
                        if sel && (up || down) {
                            resp.scroll_to_me(None);
                        }
                        if resp.clicked() {
                            run = Some(ci);
                        }
                    }
                });
            });
        });
        if esc {
            self.palette_open = false;
        }
        if let Some(ci) = run {
            self.palette_open = false;
            self.palette_query.clear();
            (cmds[ci].1)(self, ctx);
        }
    }
}

pub fn commands() -> Vec<(&'static str, Cmd)> {
    let c: Vec<(&'static str, Cmd)> = vec![
        ("Tool: Pencil", |a, _| a.tool = Tool::Pencil),
        ("Tool: Eraser", |a, _| a.tool = Tool::Eraser),
        ("Tool: Paint Bucket", |a, _| a.tool = Tool::Bucket),
        ("Tool: Eyedropper", |a, _| a.tool = Tool::Picker),
        ("Tool: Line", |a, _| a.tool = Tool::Line),
        ("Tool: Rectangle", |a, _| a.tool = Tool::Rect),
        ("Tool: Ellipse", |a, _| a.tool = Tool::Ellipse),
        ("Tool: Rectangular Marquee", |a, _| a.tool = Tool::Select),
        ("Tool: Lasso", |a, _| a.tool = Tool::Lasso),
        ("Tool: Magic Wand", |a, _| a.tool = Tool::Wand),
        ("Tool: Move", |a, _| a.tool = Tool::Move),
        ("Tool: Lighten / Darken", |a, _| a.tool = Tool::Shade),
        ("Tool: Spray", |a, _| a.tool = Tool::Spray),
        ("Tool: Text", |a, _| a.tool = Tool::Text),
        ("Tool: Hand", |a, _| a.tool = Tool::Hand),
        ("File: New...", |a, _| a.dialog = Dialog::New { w: 64, h: 64 }),
        ("File: Open...", |a, _| a.open_open()),
        ("File: Save", |a, _| match a.path.clone() {
            Some(p) => {
                if let Err(e) = a.save_to(&p) {
                    a.status = e;
                }
            }
            None => a.open_save_as(),
        }),
        ("File: Save As...", |a, _| a.open_save_as()),
        ("File: Close Tab", |a, _| a.request_close(a.active)),
        ("File: Import PNG...", |a, _| a.dialog = Dialog::Import { fb: FileBrowser { dir: home_dir(), name: String::new(), exts: vec![".png"] }, resize: true }),
        ("File: Export...", |a, _| a.open_export()),
        ("File: Start Page", |a, _| a.show_home = true),
        ("Edit: Undo", |a, _| a.do_undo()),
        ("Edit: Redo", |a, _| a.do_redo()),
        ("Edit: Cut", |a, _| a.copy_sel(true)),
        ("Edit: Copy", |a, _| a.copy_sel(false)),
        ("Edit: Paste", |a, _| a.paste()),
        ("Edit: Clear", |a, _| a.clear_sel()),
        ("Select: All", |a, _| a.select_all()),
        ("Select: None (Deselect)", |a, _| a.selection = None),
        ("Select: Invert", |a, _| a.invert_selection()),
        ("Select: Opaque Pixels", |a, _| a.select_opaque()),
        ("Select: Same Color as Primary (layer)", |a, _| {
            let c = px_of(a.color);
            let t = a.tolerance;
            a.select_color(c, t, false)
        }),
        ("Select: Same Color as Primary (flattened)", |a, _| {
            let c = px_of(a.color);
            let t = a.tolerance;
            a.select_color(c, t, true)
        }),
        ("Image: Canvas Size / Scale...", |a, _| a.dialog = Dialog::Resize { w: a.doc.w, h: a.doc.h, anchor: (1, 1), scale: false }),
        ("Image: Crop to Selection", |a, _| a.crop_to_selection()),
        ("Image: Flip Canvas Horizontal", |a, _| a.flip_canvas(true)),
        ("Image: Flip Canvas Vertical", |a, _| a.flip_canvas(false)),
        ("Image: Rotate 90 CW", |a, _| a.rotate_canvas(true)),
        ("Image: Rotate 90 CCW", |a, _| a.rotate_canvas(false)),
        ("Layer: Rotate (RotSprite)...", |a, _| a.dialog = Dialog::RotSprite { angle: 15.0, all: false }),
        ("Layer: Flip Horizontal", |a, _| a.flip_layer(true)),
        ("Layer: Flip Vertical", |a, _| a.flip_layer(false)),
        ("Layer: New", |a, _| a.layer_add()),
        ("Layer: Duplicate", |a, _| a.layer_dup()),
        ("Layer: Delete", |a, _| a.layer_del()),
        ("Layer: Merge Down", |a, _| a.layer_merge()),
        ("Layer: Flatten Image", |a, _| a.flatten()),
        ("Layer: Move Up", |a, _| a.layer_move(true)),
        ("Layer: Move Down", |a, _| a.layer_move(false)),
        ("Animation: New Frame", |a, _| a.frame_add()),
        ("Animation: Insert Frame Before", |a, _| a.frame_insert_before()),
        ("Animation: Duplicate Frame", |a, _| a.frame_dup()),
        ("Animation: Delete Frame", |a, _| a.frame_del()),
        ("Animation: Clear Frame", |a, _| a.frame_clear()),
        ("Animation: Reverse Frames", |a, _| a.frame_reverse()),
        ("Animation: Add Ping-Pong Return", |a, _| a.frame_pingpong()),
        ("Animation: Play / Stop", |a, c| a.toggle_play(c)),
        ("Animation: Next Frame", |a, _| a.goto_frame(1)),
        ("Animation: Previous Frame", |a, _| a.goto_frame(-1)),
        ("Animation: Rename Frame...", |a, _| a.dialog = Dialog::RenameFrame(a.doc.names[a.cur_frame].clone())),
        ("Animation: New Tag...", |a, _| a.dialog = dlg_new_tag(a)),
        ("Animation: Play All Frames (ignore tags)", |a, _| a.play_tag = None),
        ("Filter: Invert Colors", |a, _| a.filter_invert(false)),
        ("Filter: Grayscale", |a, _| a.filter_gray(false)),
        ("Filter: Hue / Saturation / Value...", |a, _| a.dialog = Dialog::Adjust { hue: 0.0, sat: 0.0, val: 0.0, all: false }),
        ("Filter: Outline, Shadow & Shading...", |a, _| a.dialog = dlg_effects()),
        ("Filter: Replace Color (tolerance)...", |a, _| a.dialog = Dialog::Replace { tol: 24, scope: 0 }),
        ("Filter: Snap to Palette", |a, _| a.filter_quantize(false)),
        ("Palette: Extract from Sprite", |a, _| a.extract_palette()),
        ("View: Zoom In", |a, _| a.zoom = (a.zoom + 1.0).min(64.0)),
        ("View: Zoom Out", |a, _| a.zoom = (a.zoom - 1.0).max(1.0)),
        ("View: Fit on Screen", |a, _| a.need_fit = true),
        ("View: Toggle Pixel Grid", |a, _| a.grid = !a.grid),
        ("View: Toggle Rulers", |a, _| a.rulers = !a.rulers),
        ("View: Toggle Onion Skin", |a, _| a.onion = !a.onion),
        ("View: Toggle Tiled Preview", |a, _| a.tile_preview = !a.tile_preview),
        ("View: Toggle Mirror X", |a, _| a.sym_x = !a.sym_x),
        ("View: Toggle Mirror Y", |a, _| a.sym_y = !a.sym_y),
        ("View: Light Theme", |a, c| a.set_light(c, true)),
        ("View: Dark Theme", |a, c| a.set_light(c, false)),
        ("Help: Keyboard Shortcuts", |a, _| a.dialog = Dialog::Shortcuts),
        ("Help: About SpriteCraft", |a, _| a.dialog = Dialog::About),
    ];
    c
}
