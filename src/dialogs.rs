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
                win("Export", 620.0, 470.0).show(ctx, |ui| {
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
                        let kinds = ["Current frame (PNG)", "Sprite sheet (PNG + JSON atlas)", "Animated GIF", "PNG sequence"];
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
                            if st.kind == 1 {
                                ui.add_space(10.0);
                                ui.label("Columns");
                                ui.add_sized([80.0, 28.0], egui::DragValue::new(&mut st.cols).range(1..=64));
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
                            _ => format!("{n} files, {} x {} px each", w * s, h * s),
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
                                    ("T", "Text tool (click canvas to place)"),
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
