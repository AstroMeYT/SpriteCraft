use super::*;
use eframe::egui::RichText;

const ACCENT: Color32 = Color32::from_rgb(38, 120, 220);

fn win(title: &str, w: f32, h: f32) -> egui::Window<'static> {
    egui::Window::new(RichText::new(title).strong().size(15.0))
        .collapsible(false)
        .fixed_size([w, h])
        .anchor(Align2::CENTER_CENTER, [0.0, 0.0])
}

fn roomy(ui: &mut Ui) {
    ui.spacing_mut().item_spacing = vec2(10.0, 9.0);
    ui.spacing_mut().button_padding = vec2(16.0, 6.0);
    ui.spacing_mut().interact_size.y = 28.0;
}

fn heading(ui: &mut Ui, t: &str) {
    ui.label(RichText::new(t).strong().size(15.0));
}

fn primary(ui: &mut Ui, label: &str) -> bool {
    ui.add_sized([118.0, 32.0], egui::Button::new(RichText::new(label).strong()).fill(ACCENT)).clicked()
}

fn secondary(ui: &mut Ui, label: &str) -> bool {
    ui.add_sized([96.0, 32.0], egui::Button::new(label)).clicked()
}

/// Pins the footer (buttons) to the bottom of the dialog and lets the body fill the rest.
macro_rules! layout {
    ($ui:ident, footer: $footer:block, body: $body:block) => {
        $ui.with_layout(egui::Layout::bottom_up(egui::Align::Max), |$ui| {
            $ui.add_space(4.0);
            $ui.horizontal(|$ui| $footer);
            $ui.separator();
            $ui.with_layout(egui::Layout::top_down(egui::Align::Min), |$ui| $body);
        });
    };
}

impl App {
    pub fn dialogs(&mut self, ctx: &egui::Context) {
        let mut dlg = std::mem::replace(&mut self.dialog, Dialog::None);
        let mut close = false;
        if !matches!(dlg, Dialog::None) && ctx.input(|i| i.key_pressed(Key::Escape)) {
            close = true;
        }
        match &mut dlg {
            Dialog::None => {}
            Dialog::New { w, h } => {
                win("New Sprite", 500.0, 360.0).show(ctx, |ui| {
                    roomy(ui);
                    layout!(ui, footer: {
                        if primary(ui, "Create") {
                            self.new_tab(*w, *h);
                            close = true;
                        }
                        if secondary(ui, "Cancel") {
                            close = true;
                        }
                    }, body: {
                        heading(ui, "Presets");
                        ui.horizontal(|ui| {
                            for s in [16usize, 32, 64, 128, 256] {
                                let sel = *w == s && *h == s;
                                if ui.add_sized([84.0, 34.0], egui::SelectableLabel::new(sel, format!("{s} x {s}"))).clicked() {
                                    *w = s;
                                    *h = s;
                                }
                            }
                        });
                        ui.add_space(4.0);
                        heading(ui, "Custom size");
                        ui.horizontal(|ui| {
                            ui.label("Width");
                            ui.add_sized([90.0, 28.0], egui::DragValue::new(w).range(1..=1024).suffix(" px"));
                            ui.add_space(14.0);
                            ui.label("Height");
                            ui.add_sized([90.0, 28.0], egui::DragValue::new(h).range(1..=1024).suffix(" px"));
                        });
                        ui.add_space(4.0);
                        let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), (ui.available_height() - 4.0).max(60.0)), Sense::hover());
                        let p = ui.painter_at(rect);
                        p.rect_filled(rect, 6.0, Color32::from_rgb(28, 28, 30));
                        let s = ((rect.width() - 24.0) / *w as f32).min((rect.height() - 24.0) / *h as f32).max(0.01);
                        let r = Rect::from_center_size(rect.center(), vec2(*w as f32 * s, *h as f32 * s));
                        p.rect_filled(r, 0.0, Color32::from_gray(70));
                        p.rect_stroke(r, 0.0, Stroke::new(1.5, ACCENT));
                        p.text(rect.right_bottom() - vec2(10.0, 8.0), Align2::RIGHT_BOTTOM, format!("{} x {} px  -  {} pixels", w, h, *w * *h), FontId::proportional(12.0), Color32::from_gray(170));
                    });
                });
            }
            Dialog::Open(fb) => {
                win("Open Project", 680.0, 500.0).show(ctx, |ui| {
                    roomy(ui);
                    layout!(ui, footer: {
                        if primary(ui, "Open") {
                            let p = fb.dir.join(&fb.name);
                            match self.load_from(&p) {
                                Ok(()) => close = true,
                                Err(e) => self.status = e,
                            }
                        }
                        if secondary(ui, "Cancel") {
                            close = true;
                        }
                        ui.label(RichText::new(&self.status).small().weak());
                    }, body: {
                        browser_ui(ui, fb, true);
                    });
                });
            }
            Dialog::Save(fb) => {
                win("Save Project", 680.0, 500.0).show(ctx, |ui| {
                    roomy(ui);
                    layout!(ui, footer: {
                        if primary(ui, "Save") {
                            let _ = std::fs::create_dir_all(&fb.dir);
                            let p = fb.dir.join(sanitize_ext(&fb.name, "spc"));
                            match self.save_to(&p) {
                                Ok(()) => close = true,
                                Err(e) => self.status = e,
                            }
                        }
                        if secondary(ui, "Cancel") {
                            close = true;
                        }
                        ui.label(RichText::new(&self.status).small().weak());
                    }, body: {
                        browser_ui(ui, fb, true);
                    });
                });
            }
            Dialog::Import { fb, resize } => {
                win("Import PNG", 680.0, 540.0).show(ctx, |ui| {
                    roomy(ui);
                    layout!(ui, footer: {
                        if primary(ui, "Import") {
                            let p = fb.dir.join(&fb.name);
                            match self.import_png(&p, *resize) {
                                Ok(()) => close = true,
                                Err(e) => self.status = e,
                            }
                        }
                        if secondary(ui, "Cancel") {
                            close = true;
                        }
                        ui.label(RichText::new(&self.status).small().weak());
                    }, body: {
                        ui.checkbox(resize, "Create a new sprite sized to the image (otherwise paste into the current layer)");
                        browser_ui(ui, fb, true);
                    });
                });
            }
            Dialog::Export(st) => {
                win("Export", 680.0, 500.0).show(ctx, |ui| {
                    roomy(ui);
                    let (w, h, n) = (self.doc.w, self.doc.h, self.doc.frames.len());
                    layout!(ui, footer: {
                        if primary(ui, "Export") {
                            match self.export(st) {
                                Ok(p) => {
                                    self.status = format!("Exported {p}");
                                    close = true;
                                }
                                Err(e) => self.status = e,
                            }
                        }
                        if secondary(ui, "Cancel") {
                            close = true;
                        }
                        ui.label(RichText::new(&self.status).small().weak());
                    }, body: {
                        let kinds = [
                            "Current frame (PNG)",
                            "Sprite sheet (PNG + simple JSON atlas)",
                            "Animated GIF",
                            "PNG sequence",
                            "Godot 4 - SpriteFrames (.tres + sheet)",
                            "Aseprite-style JSON + sheet (Phaser, PixiJS, Defold...)",
                            "Horizontal strip PNG (GameMaker, RPG Maker, Unity slicing)",
                            "CSS / HTML animation snippet",
                        ];
                        heading(ui, "Format");
                        let fw = ui.available_width();
                        egui::ComboBox::from_id_salt("kind").width(fw - 8.0).selected_text(kinds[st.kind]).show_ui(ui, |ui| {
                            for (i, k) in kinds.iter().enumerate() {
                                ui.selectable_value(&mut st.kind, i, *k);
                            }
                        });
                        heading(ui, "Options");
                        ui.horizontal(|ui| {
                            ui.label("Scale");
                            ui.add_sized([100.0, 28.0], egui::DragValue::new(&mut st.scale).range(1..=32).prefix("x"));
                            if matches!(st.kind, 1 | 4 | 5) {
                                ui.add_space(10.0);
                                ui.label("Columns");
                                ui.add_sized([80.0, 28.0], egui::DragValue::new(&mut st.cols).range(1..=64));
                            }
                            if st.kind == 1 {
                                ui.checkbox(&mut st.json, "JSON atlas");
                            }
                        });
                        heading(ui, "Destination");
                        let tw = (ui.available_width() - 90.0).max(160.0);
                        egui::Grid::new("exgrid").num_columns(2).spacing([12.0, 10.0]).show(ui, |ui| {
                            ui.label("Folder");
                            ui.add(egui::TextEdit::singleline(&mut st.dir).desired_width(tw));
                            ui.end_row();
                            ui.label("Name");
                            ui.add(egui::TextEdit::singleline(&mut st.name).desired_width(tw));
                            ui.end_row();
                        });
                        let s = st.scale.max(1) as usize;
                        let summary = match st.kind {
                            0 => format!("1 image, {} x {} px", w * s, h * s),
                            1 => {
                                let cols = st.cols.clamp(1, n);
                                format!("{} x {} px sheet ({} frames, {} columns)", w * s * cols, h * s * ((n + cols - 1) / cols), n, cols)
                            }
                            2 => format!("{n} frames at {} fps, {} x {} px", self.doc.fps, w * s, h * s),
                            3 => format!("{n} files, {} x {} px each", w * s, h * s),
                            4 | 5 => {
                                let cols = st.cols.clamp(1, n);
                                format!("{} x {} px sheet, {n} frames, {} tag(s) exported as animations", w * s * cols, h * s * ((n + cols - 1) / cols), self.doc.tags.len())
                            }
                            6 => format!("{} x {} px strip ({n} frames of {} x {})", w * s * n, h * s, w * s, h * s),
                            _ => format!("strip PNG + HTML/CSS keyframes, {n} frames of {} x {}", w * s, h * s),
                        };
                        ui.add_space(4.0);
                        ui.label(RichText::new(summary).color(Color32::from_gray(170)));
                    });
                });
            }
            Dialog::Resize { w, h, anchor, scale } => {
                win("Canvas Size / Scale", 540.0, 450.0).show(ctx, |ui| {
                    roomy(ui);
                    let (cw, ch) = (self.doc.w, self.doc.h);
                    layout!(ui, footer: {
                        if primary(ui, "Apply") {
                            self.push_undo();
                            if *scale {
                                self.doc.scale_canvas(*w, *h);
                            } else {
                                let ox = anchor.0 * (*w as i32 - self.doc.w as i32) / 2;
                                let oy = anchor.1 * (*h as i32 - self.doc.h as i32) / 2;
                                self.doc.resize_canvas(*w, *h, ox, oy);
                            }
                            self.selection = None;
                            self.need_fit = true;
                            self.changed_all();
                            close = true;
                        }
                        if secondary(ui, "Cancel") {
                            close = true;
                        }
                    }, body: {
                        ui.horizontal(|ui| {
                            let bw = (ui.available_width() - 10.0) / 2.0;
                            if ui.add_sized([bw, 34.0], egui::SelectableLabel::new(!*scale, "Canvas size (crop / extend)")).clicked() {
                                *scale = false;
                            }
                            if ui.add_sized([bw, 34.0], egui::SelectableLabel::new(*scale, "Scale (nearest neighbor)")).clicked() {
                                *scale = true;
                            }
                        });
                        heading(ui, "New size");
                        ui.horizontal(|ui| {
                            ui.label("Width");
                            ui.add_sized([100.0, 28.0], egui::DragValue::new(w).range(1..=2048).suffix(" px"));
                            ui.add_space(14.0);
                            ui.label("Height");
                            ui.add_sized([100.0, 28.0], egui::DragValue::new(h).range(1..=2048).suffix(" px"));
                        });
                        ui.label(RichText::new(format!("Current: {cw} x {ch} px   ->   New: {} x {} px", w, h)).color(Color32::from_gray(170)));
                        if !*scale {
                            heading(ui, "Anchor");
                            egui::Grid::new("anchor").spacing([6.0, 6.0]).show(ui, |ui| {
                                for ay in 0..3 {
                                    for ax in 0..3 {
                                        let sel = *anchor == (ax, ay);
                                        if ui.add_sized([52.0, 36.0], egui::SelectableLabel::new(sel, if sel { "O" } else { "." })).clicked() {
                                            *anchor = (ax, ay);
                                        }
                                    }
                                    ui.end_row();
                                }
                            });
                        }
                    });
                });
            }
            Dialog::Adjust { hue, sat, val, all } => {
                win("Hue / Saturation / Value", 560.0, 340.0).show(ctx, |ui| {
                    roomy(ui);
                    layout!(ui, footer: {
                        if primary(ui, "Apply") {
                            self.filter_hsv(*all, *hue, *sat, *val);
                            close = true;
                        }
                        if secondary(ui, "Cancel") {
                            close = true;
                        }
                        if secondary(ui, "Reset") {
                            *hue = 0.0;
                            *sat = 0.0;
                            *val = 0.0;
                        }
                    }, body: {
                        ui.spacing_mut().slider_width = (ui.available_width() - 150.0).max(120.0);
                        ui.add(egui::Slider::new(hue, -180.0..=180.0).text("Hue"));
                        ui.add(egui::Slider::new(sat, -100.0..=100.0).text("Saturation"));
                        ui.add(egui::Slider::new(val, -100.0..=100.0).text("Value"));
                        ui.add_space(6.0);
                        ui.checkbox(all, "Apply to all frames");
                        ui.label(RichText::new("Applies to the current layer and respects the active selection.").small().weak());
                    });
                });
            }
            Dialog::Shortcuts => {
                win("Keyboard Shortcuts", 640.0, 580.0).show(ctx, |ui| {
                    roomy(ui);
                    layout!(ui, footer: {
                        if primary(ui, "Close") {
                            close = true;
                        }
                    }, body: {
                        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                            egui::Grid::new("sc").num_columns(2).spacing([40.0, 7.0]).striped(true).show(ui, |ui| {
                                for (k, d) in [
                                    ("B / E / G / I", "Pencil / Eraser / Bucket / Eyedropper"),
                                    ("L / R / O", "Line / Rectangle / Ellipse"),
                                    ("M / W / V", "Marquee / Magic wand / Move"),
                                    ("D / A / H", "Lighten-Darken / Spray / Hand"),
                                    ("T / Q", "Text tool / Lasso"),
                                    ("Ctrl+K", "Command palette (search every action)"),
                                    ("[ and ]", "Brush size"),
                                    ("X", "Swap colors"),
                                    ("Right-click", "Draw with secondary color"),
                                    ("Alt+click", "Eyedropper while drawing"),
                                    ("Shift", "Constrain shapes / add to selection"),
                                    ("Space + drag / middle drag", "Pan"),
                                    ("Scroll wheel", "Zoom at cursor"),
                                    ("Ctrl+Z / Ctrl+Shift+Z", "Undo / Redo"),
                                    ("Ctrl+C / X / V", "Copy / Cut / Paste"),
                                    ("Ctrl+A / Ctrl+D", "Select all / Deselect"),
                                    ("Del", "Clear selection or layer"),
                                    ("Ctrl+N / O / S / E", "New / Open / Save / Export"),
                                    ("Ctrl+W", "Close tab"),
                                    ("Ctrl+G / Ctrl+0", "Toggle grid / Fit on screen"),
                                    (", and .", "Previous / next frame"),
                                    ("Enter", "Play / stop"),
                                    ("Right-click a frame", "Frame menu"),
                                ] {
                                    ui.label(RichText::new(k).monospace().strong());
                                    ui.label(d);
                                    ui.end_row();
                                }
                            });
                        });
                    });
                });
            }
            Dialog::About => {
                win("About SpriteCraft", 540.0, 440.0).show(ctx, |ui| {
                    roomy(ui);
                    layout!(ui, footer: {
                        if primary(ui, "Close") {
                            close = true;
                        }
                    }, body: {
                        ui.vertical_centered(|ui| {
                            ui.add_space(6.0);
                            if let Some(t) = &self.logo_tex {
                                ui.image((t.id(), vec2(112.0, 112.0)));
                            }
                            ui.add_space(4.0);
                            ui.label(RichText::new("SpriteCraft").strong().size(28.0));
                            ui.label(RichText::new(format!("Version {}", env!("CARGO_PKG_VERSION"))).color(Color32::from_gray(170)));
                            ui.add_space(8.0);
                            ui.label("2D pixel art and animation studio");
                            ui.label(RichText::new("Layers - frames - onion skin - palettes - filters - text - project tabs").small().weak());
                            ui.label(RichText::new("Export to PNG, sprite sheet + JSON atlas, GIF and PNG sequences").small().weak());
                            ui.add_space(8.0);
                            ui.label(RichText::new("Written in Rust with egui").small().weak());
                        });
                    });
                });
            }
            Dialog::Effects { tab, thick, diag, inner, sdx, sdy, light, depth, strength, hue, all } => {
                win("Outline, Shadow & Shading", 620.0, 560.0).show(ctx, |ui| {
                    roomy(ui);
                    layout!(ui, footer: {
                        if primary(ui, "Apply") {
                            match *tab {
                                0 => self.fx_outline(*all, *thick, *diag, *inner),
                                1 => self.fx_shadow(*all, *sdx, *sdy),
                                _ => self.fx_shade(*all, *light, *depth, *strength, *hue),
                            }
                            close = true;
                        }
                        if secondary(ui, "Cancel") {
                            close = true;
                        }
                    }, body: {
                        ui.horizontal(|ui| {
                            let bw = (ui.available_width() - 20.0) / 3.0;
                            for (i, n) in ["Outline", "Drop shadow", "Auto-shade"].iter().enumerate() {
                                if ui.add_sized([bw, 34.0], egui::SelectableLabel::new(*tab == i, *n)).clicked() {
                                    *tab = i;
                                }
                            }
                        });
                        ui.add_space(4.0);
                        ui.spacing_mut().slider_width = (ui.available_width() - 160.0).max(120.0);
                        match *tab {
                            0 => {
                                ui.label(RichText::new("Uses the primary color.").color(self.th.dim));
                                ui.add(egui::Slider::new(thick, 1..=4).text("Thickness (px)"));
                                ui.checkbox(diag, "Include diagonals (8-neighbour, rounder corners)");
                                ui.checkbox(inner, "Inner outline (recolor the sprite's own edge)");
                            }
                            1 => {
                                ui.label(RichText::new("Uses the secondary color for the shadow.").color(self.th.dim));
                                ui.add(egui::Slider::new(sdx, -8..=8).text("Offset X"));
                                ui.add(egui::Slider::new(sdy, -8..=8).text("Offset Y"));
                            }
                            _ => {
                                ui.label(RichText::new("Adds highlights on edges facing the light and hue-shifted shadows opposite.").color(self.th.dim));
                                ui.horizontal(|ui| {
                                    ui.label("Light from");
                                    egui::Grid::new("lightdir").spacing([4.0, 4.0]).show(ui, |ui| {
                                        let dirs = [("NW", (-1, -1)), ("N", (0, -1)), ("NE", (1, -1)), ("W", (-1, 0)), ("", (0, 0)), ("E", (1, 0)), ("SW", (-1, 1)), ("S", (0, 1)), ("SE", (1, 1))];
                                        for (i, (n, d)) in dirs.iter().enumerate() {
                                            if d.0 == 0 && d.1 == 0 {
                                                ui.label("");
                                            } else if ui.add_sized([46.0, 30.0], egui::SelectableLabel::new(*light == *d, *n)).clicked() {
                                                *light = *d;
                                            }
                                            if i % 3 == 2 {
                                                ui.end_row();
                                            }
                                        }
                                    });
                                });
                                ui.add(egui::Slider::new(depth, 1..=4).text("Depth (px)"));
                                ui.add(egui::Slider::new(strength, 0.1..=1.0).text("Strength"));
                                ui.add(egui::Slider::new(hue, 0.0..=1.0).text("Hue shift"));
                            }
                        }
                        ui.add_space(6.0);
                        ui.checkbox(all, "Apply to all frames (current layer)");
                        ui.label(RichText::new("Respects the active selection.").small().weak());
                    });
                });
            }
            Dialog::RotSprite { angle, all } => {
                win("Rotate (RotSprite)", 580.0, 380.0).show(ctx, |ui| {
                    roomy(ui);
                    layout!(ui, footer: {
                        if primary(ui, "Rotate") {
                            self.rotate_layer(*angle, *all);
                            close = true;
                        }
                        if secondary(ui, "Cancel") {
                            close = true;
                        }
                    }, body: {
                        ui.spacing_mut().slider_width = (ui.available_width() - 170.0).max(120.0);
                        ui.add(egui::Slider::new(angle, -180.0..=180.0).text("Angle").suffix(" deg"));
                        ui.horizontal(|ui| {
                            for a in [-90.0f32, -45.0, -15.0, 15.0, 45.0, 90.0, 180.0] {
                                if ui.add_sized([62.0, 30.0], egui::Button::new(format!("{a:+}"))).clicked() {
                                    *angle = a;
                                }
                            }
                        });
                        ui.add_space(6.0);
                        ui.checkbox(all, "Rotate this layer in all frames");
                        ui.label(RichText::new("RotSprite scales the art 8x with Scale2x, rotates, and samples back down so edges stay crisp. Rotates the selection around its center, or the whole layer around the canvas center. Positive = clockwise.").small().color(self.th.dim));
                    });
                });
            }
            Dialog::Replace { tol, scope } => {
                win("Replace Color", 580.0, 420.0).show(ctx, |ui| {
                    roomy(ui);
                    layout!(ui, footer: {
                        if primary(ui, "Replace") {
                            self.fx_replace(*tol, *scope);
                            close = true;
                        }
                        if secondary(ui, "Cancel") {
                            close = true;
                        }
                    }, body: {
                        ui.horizontal(|ui| {
                            for (c, l) in [(self.color, "Primary (find)"), (self.color2, "Secondary (replace with)")] {
                                let (r, _) = ui.allocate_exact_size(vec2(56.0, 40.0), Sense::hover());
                                ui.painter().rect_filled(r, 4.0, c);
                                ui.painter().rect_stroke(r, 4.0, Stroke::new(1.0, self.th.border));
                                ui.label(l);
                                ui.add_space(12.0);
                            }
                        });
                        ui.spacing_mut().slider_width = (ui.available_width() - 160.0).max(120.0);
                        ui.add(egui::Slider::new(tol, 0..=255).text("Tolerance"));
                        ui.label(RichText::new("0 = exact match. Higher values also catch similar shades.").small().color(self.th.dim));
                        heading(ui, "Scope");
                        for (i, n) in ["This layer, this frame", "All layers, this frame", "This layer, all frames", "Everything"].iter().enumerate() {
                            ui.radio_value(scope, i, *n);
                        }
                    });
                });
            }
            Dialog::Tag { idx, name, from, to, dir, color } => {
                let n = self.doc.frames.len();
                win(if idx.is_some() { "Edit Tag" } else { "New Tag" }, 560.0, 430.0).show(ctx, |ui| {
                    roomy(ui);
                    layout!(ui, footer: {
                        if primary(ui, "Save") {
                            self.tag_apply(*idx, name.clone(), *from, *to, *dir, *color);
                            close = true;
                        }
                        if secondary(ui, "Cancel") {
                            close = true;
                        }
                        if let Some(i) = *idx {
                            if ui.add_sized([96.0, 32.0], egui::Button::new("Delete")).clicked() {
                                self.tag_delete(i);
                                close = true;
                            }
                        }
                    }, body: {
                        heading(ui, "Name");
                        ui.add(egui::TextEdit::singleline(name).desired_width(ui.available_width() - 8.0));
                        heading(ui, "Frames");
                        ui.horizontal(|ui| {
                            let (mut a, mut b) = (*from + 1, *to + 1);
                            ui.label("From");
                            ui.add_sized([80.0, 28.0], egui::DragValue::new(&mut a).range(1..=n));
                            ui.label("To");
                            ui.add_sized([80.0, 28.0], egui::DragValue::new(&mut b).range(1..=n));
                            *from = a - 1;
                            *to = b - 1;
                        });
                        heading(ui, "Direction");
                        ui.horizontal(|ui| {
                            for (i, l) in ["Forward", "Reverse", "Ping-pong"].iter().enumerate() {
                                ui.radio_value(dir, i, *l);
                            }
                        });
                        heading(ui, "Color");
                        ui.horizontal(|ui| {
                            for c in [[38u8, 120, 220], [220, 70, 70], [70, 180, 90], [230, 170, 50], [160, 90, 200], [60, 190, 200], [230, 110, 170], [140, 140, 150]] {
                                let (r, resp) = ui.allocate_exact_size(vec2(30.0, 30.0), Sense::click());
                                ui.painter().rect_filled(r, 5.0, Color32::from_rgb(c[0], c[1], c[2]));
                                if *color == c {
                                    ui.painter().rect_stroke(r, 5.0, Stroke::new(2.5, self.th.strong));
                                }
                                if resp.clicked() {
                                    *color = c;
                                }
                            }
                        });
                    });
                });
            }
            Dialog::RenameFrame(name) => {
                win("Rename Frame", 480.0, 210.0).show(ctx, |ui| {
                    roomy(ui);
                    let fi = self.cur_frame;
                    layout!(ui, footer: {
                        if primary(ui, "OK") {
                            self.rename_frame(name.clone());
                            close = true;
                        }
                        if secondary(ui, "Cancel") {
                            close = true;
                        }
                    }, body: {
                        ui.label(format!("Name for frame {}:", fi + 1));
                        let r = ui.add(egui::TextEdit::singleline(name).desired_width(ui.available_width() - 8.0).hint_text("e.g. contact, recovery, idle-1"));
                        r.request_focus();
                        if r.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter)) {
                            self.rename_frame(name.clone());
                            close = true;
                        }
                    });
                });
            }
            Dialog::Recover(list) => {
                win("Recover unsaved work?", 620.0, 420.0).show(ctx, |ui| {
                    roomy(ui);
                    let items = list.clone();
                    layout!(ui, footer: {
                        if primary(ui, "Recover") {
                            self.recover(&items);
                            close = true;
                        }
                        if secondary(ui, "Discard") {
                            self.discard_recovery(&items);
                            close = true;
                        }
                    }, body: {
                        ui.label(RichText::new("SpriteCraft did not close cleanly last time. These projects were autosaved:").size(14.0));
                        ui.add_space(4.0);
                        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                            for c in &items {
                                for (f, orig) in &c.files {
                                    let name = orig.clone().unwrap_or_else(|| "Untitled (never saved)".into());
                                    let when = std::fs::metadata(f).and_then(|m| m.modified()).ok().and_then(|t| t.elapsed().ok()).map(|d| format!("{} min ago", d.as_secs() / 60)).unwrap_or_default();
                                    ui.label(RichText::new(name).strong());
                                    ui.label(RichText::new(format!("autosaved {when}")).small().color(self.th.dim));
                                    ui.separator();
                                }
                            }
                        });
                    });
                });
            }
            Dialog::ConfirmQuit => {
                win("Quit SpriteCraft?", 500.0, 210.0).show(ctx, |ui| {
                    roomy(ui);
                    layout!(ui, footer: {
                        if ui.add_sized([170.0, 32.0], egui::Button::new(RichText::new("Quit without saving").strong()).fill(Color32::from_rgb(170, 50, 50))).clicked() {
                            self.quit_ok = true;
                            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                            close = true;
                        }
                        if secondary(ui, "Cancel") {
                            close = true;
                        }
                    }, body: {
                        ui.add_space(8.0);
                        ui.label(RichText::new("You have projects with unsaved changes.").size(15.0));
                        ui.label(RichText::new("They will be lost if you quit now.").color(self.th.dim));
                    });
                });
            }
            Dialog::ConfirmClose(i) => {
                let idx = *i;
                win("Unsaved changes", 480.0, 200.0).show(ctx, |ui| {
                    roomy(ui);
                    if idx >= self.tabs.len() {
                        close = true;
                        return;
                    }
                    let name = self.tab_title(idx).0;
                    layout!(ui, footer: {
                        if ui.add_sized([170.0, 32.0], egui::Button::new(RichText::new("Close without saving").strong()).fill(Color32::from_rgb(170, 50, 50))).clicked() {
                            self.close_tab(idx);
                            close = true;
                        }
                        if secondary(ui, "Cancel") {
                            close = true;
                        }
                    }, body: {
                        ui.add_space(8.0);
                        ui.label(RichText::new(format!("\"{name}\" has unsaved changes.")).size(15.0));
                        ui.label(RichText::new("If you close this tab they will be lost.").color(Color32::from_gray(170)));
                    });
                });
            }
        }
        if !close && matches!(self.dialog, Dialog::None) {
            self.dialog = dlg;
        }
    }
}
