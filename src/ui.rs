use super::*;
use eframe::egui::{Frame, Modifiers, PointerButton, RichText, ScrollArea};

const ACCENT: Color32 = Color32::from_rgb(38, 120, 220);

fn mi(ui: &mut Ui, label: &str, sc: &str) -> bool {
    let r = ui.add(egui::Button::new(label).shortcut_text(sc));
    if r.clicked() {
        ui.close_menu();
        true
    } else {
        false
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _f: &mut eframe::Frame) {
        self.shift = ctx.input(|i| i.modifiers.shift);
        let now = ctx.input(|i| i.time);
        if self.last_autosave == 0.0 {
            self.last_autosave = now.max(0.001);
            self.startup();
        }
        if ctx.input(|i| i.viewport().close_requested()) && self.any_modified() && !self.quit_ok {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            self.dialog = Dialog::ConfirmQuit;
        }
        self.shortcuts(ctx);
        self.tick_playback(ctx);
        self.refresh_textures(ctx);
        self.menu_bar(ctx);
        let home = self.show_home;
        if !home {
            self.options_bar(ctx);
        }
        self.status_bar(ctx);
        if home {
            self.start_page(ctx);
        } else {
            self.toolbox(ctx);
            self.right_panel(ctx);
            self.timeline(ctx);
            self.canvas_panel(ctx);
        }
        self.dialogs(ctx);
        self.command_palette(ctx);
        self.handle_drops(ctx);
        self.window_chrome(ctx);
        self.autosave(now);
        ctx.request_repaint_after(std::time::Duration::from_secs(30));
        if self.dirty || self.dirty_all {
            ctx.request_repaint();
        }
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        let _ = std::fs::remove_dir_all(config::session_dir(&self.session));
    }
}

impl App {
    // ------------------------------------------------------------ housekeeping
    pub fn toggle_play(&mut self, ctx: &egui::Context) {
        self.playing = !self.playing;
        let seq = self.playback_seq();
        self.play_pos = seq.iter().position(|&f| f == self.cur_frame).unwrap_or(0);
        self.play_frame = seq[self.play_pos];
        self.play_last = ctx.input(|i| i.time);
    }

    fn tick_playback(&mut self, ctx: &egui::Context) {
        if !self.playing {
            return;
        }
        let now = ctx.input(|i| i.time);
        let seq = self.playback_seq();
        self.play_pos = self.play_pos.min(seq.len() - 1);
        self.play_frame = seq[self.play_pos];
        let dur = self.doc.holds[self.play_frame].max(1) as f64 / self.doc.fps.max(1) as f64;
        if now - self.play_last >= dur {
            self.play_pos = (self.play_pos + 1) % seq.len();
            self.play_frame = seq[self.play_pos];
            self.play_last = now;
        }
        ctx.request_repaint();
    }

    fn update_thumb(&mut self, ctx: &egui::Context, i: usize) {
        let comp = self.doc.composite(i);
        let img = to_color_image(self.doc.w, self.doc.h, &comp);
        match &mut self.thumbs[i] {
            Some(t) => t.set(img, TextureOptions::NEAREST),
            None => self.thumbs[i] = Some(ctx.load_texture(format!("thumb{i}"), img, TextureOptions::NEAREST)),
        }
    }

    fn refresh_textures(&mut self, ctx: &egui::Context) {
        let disp = if self.playing { self.play_frame } else { self.cur_frame };
        let comp = self.doc.composite(disp);
        let img = to_color_image(self.doc.w, self.doc.h, &comp);
        match &mut self.prev_tex {
            Some(t) => t.set(img, TextureOptions::NEAREST),
            None => self.prev_tex = Some(ctx.load_texture("preview", img, TextureOptions::NEAREST)),
        }
        let n = self.doc.frames.len();
        if self.thumbs.len() != n {
            self.thumbs.resize(n, None);
            self.dirty_all = true;
        }
        if self.dirty_all {
            for i in 0..n {
                self.update_thumb(ctx, i);
            }
        } else if self.dirty {
            self.update_thumb(ctx, self.cur_frame.min(n - 1));
        }
        self.dirty = false;
        self.dirty_all = false;
    }

    fn build_main_image(&self) -> egui::ColorImage {
        let (w, h) = (self.doc.w, self.doc.h);
        let disp = if self.playing { self.play_frame } else { self.cur_frame };
        let n = self.doc.frames.len();
        let mut out = vec![CLEAR; w * h];
        if self.onion && !self.playing {
            for k in (1..=self.onion_n).rev() {
                if disp >= k {
                    blend_onion(&mut out, &self.doc.composite(disp - k), [255, 80, 80], 0.38);
                }
            }
            for k in (1..=self.onion_n).rev() {
                if disp + k < n {
                    blend_onion(&mut out, &self.doc.composite(disp + k), [70, 230, 110], 0.38);
                }
            }
        }
        let comp = self.doc.composite(disp);
        for (o, s) in out.iter_mut().zip(comp.iter()) {
            if s[3] != 0 {
                *o = over(*o, *s, 1.0);
            }
        }
        to_color_image(w, h, &out)
    }

    // ------------------------------------------------------------ shortcuts
    fn shortcuts(&mut self, ctx: &egui::Context) {
        if ctx.wants_keyboard_input() {
            return;
        }
        let k = |m: Modifiers, key: Key| ctx.input_mut(|i| i.consume_key(m, key));
        let cs = Modifiers::COMMAND | Modifiers::SHIFT;
        if k(cs, Key::Z) || k(Modifiers::COMMAND, Key::Y) {
            self.do_redo();
        }
        if k(Modifiers::COMMAND, Key::Z) {
            self.do_undo();
        }
        if k(Modifiers::COMMAND, Key::C) {
            self.copy_sel(false);
        }
        if k(Modifiers::COMMAND, Key::X) {
            self.copy_sel(true);
        }
        if k(Modifiers::COMMAND, Key::V) {
            self.paste();
        }
        if k(Modifiers::COMMAND, Key::A) {
            self.select_all();
        }
        if k(Modifiers::COMMAND, Key::D) {
            self.selection = None;
        }
        if k(cs, Key::S) {
            self.open_save_as();
        } else if k(Modifiers::COMMAND, Key::S) {
            match self.path.clone() {
                Some(p) => {
                    if let Err(e) = self.save_to(&p) {
                        self.status = e;
                    }
                }
                None => self.open_save_as(),
            }
        }
        if k(Modifiers::COMMAND, Key::O) {
            self.open_open();
        }
        if k(Modifiers::COMMAND, Key::N) {
            self.dialog = Dialog::New { w: 64, h: 64 };
        }
        if k(Modifiers::COMMAND, Key::E) {
            self.open_export();
        }
        if k(Modifiers::COMMAND, Key::K) || k(cs, Key::P) {
            self.palette_open = true;
            self.palette_query.clear();
            self.palette_sel = 0;
        }
        if k(Modifiers::COMMAND, Key::W) {
            self.request_close(self.active);
        }
        if k(Modifiers::COMMAND, Key::G) {
            self.grid = !self.grid;
        }
        if k(Modifiers::COMMAND, Key::Num0) {
            self.need_fit = true;
        }
        let pressed = |key: Key| ctx.input(|i| i.key_pressed(key) && !i.modifiers.command && !i.modifiers.alt);
        for (t, _, key) in TOOLS {
            let kk = match *key {
                "B" => Key::B, "E" => Key::E, "G" => Key::G, "I" => Key::I, "L" => Key::L, "R" => Key::R, "O" => Key::O,
                "M" => Key::M, "W" => Key::W, "V" => Key::V, "D" => Key::D, "A" => Key::A, "T" => Key::T, "Q" => Key::Q, _ => Key::H,
            };
            if pressed(kk) {
                self.tool = *t;
            }
        }
        if pressed(Key::X) {
            std::mem::swap(&mut self.color, &mut self.color2);
        }
        if pressed(Key::OpenBracket) {
            self.brush = (self.brush - 1).max(1);
        }
        if pressed(Key::CloseBracket) {
            self.brush = (self.brush + 1).min(32);
        }
        if pressed(Key::Comma) {
            self.goto_frame(-1);
        }
        if pressed(Key::Period) {
            self.goto_frame(1);
        }
        if pressed(Key::Enter) {
            self.toggle_play(ctx);
        }
        if pressed(Key::Delete) || pressed(Key::Backspace) {
            self.clear_sel();
        }
        if pressed(Key::Plus) || pressed(Key::Equals) {
            self.zoom = (self.zoom + 1.0).min(64.0);
        }
        if pressed(Key::Minus) {
            self.zoom = (self.zoom - 1.0).max(1.0);
        }
        if pressed(Key::Escape) {
            self.selection = None;
        }
    }

    pub fn open_open(&mut self) {
        self.dialog = Dialog::Open(FileBrowser { dir: projects_dir(), name: String::new(), exts: vec![".spc"] });
    }

    pub fn open_save_as(&mut self) {
        let (dir, name) = match &self.path {
            Some(p) => (p.parent().map(|d| d.to_path_buf()).unwrap_or_else(projects_dir), p.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default()),
            None => (projects_dir(), "sprite.spc".to_string()),
        };
        self.dialog = Dialog::Save(FileBrowser { dir, name, exts: vec![".spc"] });
    }

    pub fn open_export(&mut self) {
        let stem = self.path.as_ref().and_then(|p| p.file_stem()).map(|s| s.to_string_lossy().to_string()).unwrap_or_else(|| "sprite".into());
        let dir = projects_dir().join("exports").to_string_lossy().to_string();
        self.dialog = Dialog::Export(ExportState { dir, name: stem, kind: 0, scale: 4, cols: 8, json: true });
    }

    // ------------------------------------------------------------ menu bar
    fn menu_bar(&mut self, ctx: &egui::Context) {
        let frame = Frame::none().fill(self.th.title).inner_margin(egui::Margin::symmetric(6.0, 0.0));
        egui::TopBottomPanel::top("menu").frame(frame).exact_height(32.0).show(ctx, |ui| {
            ui.spacing_mut().interact_size.y = 32.0;
            ui.spacing_mut().button_padding = vec2(9.0, 6.0);
            ui.visuals_mut().widgets.inactive.weak_bg_fill = Color32::TRANSPARENT;
            ui.visuals_mut().widgets.inactive.bg_fill = Color32::TRANSPARENT;
            egui::menu::bar(ui, |ui| {
                if let Some(t) = &self.logo_tex {
                    ui.add_space(2.0);
                    ui.image((t.id(), vec2(22.0, 22.0)));
                    ui.add_space(8.0);
                }
                ui.menu_button("File", |ui| {
                    if mi(ui, "New...", "Ctrl+N") {
                        self.dialog = Dialog::New { w: 64, h: 64 };
                    }
                    if mi(ui, "Open...", "Ctrl+O") {
                        self.open_open();
                    }
                    if mi(ui, "Save", "Ctrl+S") {
                        match self.path.clone() {
                            Some(p) => {
                                if let Err(e) = self.save_to(&p) {
                                    self.status = e;
                                }
                            }
                            None => self.open_save_as(),
                        }
                    }
                    if mi(ui, "Save As...", "Ctrl+Shift+S") {
                        self.open_save_as();
                    }
                    if mi(ui, "Close Tab", "Ctrl+W") {
                        self.request_close(self.active);
                    }
                    if mi(ui, "Start Page", "") {
                        self.show_home = true;
                    }
                    ui.menu_button("Open Recent", |ui| {
                        if self.config.recent.is_empty() {
                            ui.label("No recent files");
                        }
                        for r in self.config.recent.clone() {
                            let p = PathBuf::from(&r);
                            let name = p.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or(r.clone());
                            if ui.button(name).on_hover_text(&r).clicked() {
                                self.open_path(&p);
                                ui.close_menu();
                            }
                        }
                    });
                    ui.separator();
                    if mi(ui, "Import PNG...", "") {
                        self.dialog = Dialog::Import { fb: FileBrowser { dir: home_dir(), name: String::new(), exts: vec![".png"] }, resize: true };
                    }
                    if mi(ui, "Export...", "Ctrl+E") {
                        self.open_export();
                    }
                    ui.separator();
                    if mi(ui, "Quit", "") {
                        ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
                    }
                });
                ui.menu_button("Edit", |ui| {
                    if mi(ui, "Undo", "Ctrl+Z") {
                        self.do_undo();
                    }
                    if mi(ui, "Redo", "Ctrl+Shift+Z") {
                        self.do_redo();
                    }
                    ui.separator();
                    if mi(ui, "Cut", "Ctrl+X") {
                        self.copy_sel(true);
                    }
                    if mi(ui, "Copy", "Ctrl+C") {
                        self.copy_sel(false);
                    }
                    if mi(ui, "Paste", "Ctrl+V") {
                        self.paste();
                    }
                    if mi(ui, "Clear", "Del") {
                        self.clear_sel();
                    }
                    ui.separator();
                    if mi(ui, "Select All", "Ctrl+A") {
                        self.select_all();
                    }
                    if mi(ui, "Deselect", "Ctrl+D") {
                        self.selection = None;
                    }
                    if mi(ui, "Invert Selection", "") {
                        self.invert_selection();
                    }
                    if mi(ui, "Select Opaque Pixels", "") {
                        self.select_opaque();
                    }
                    if mi(ui, "Select Same Color as Primary", "") {
                        let (c, t) = (px_of(self.color), self.tolerance);
                        self.select_color(c, t, false);
                    }
                    if mi(ui, "Command Palette...", "Ctrl+K") {
                        self.palette_open = true;
                        self.palette_query.clear();
                    }
                });
                ui.menu_button("Image", |ui| {
                    if mi(ui, "Canvas Size / Scale...", "") {
                        self.dialog = Dialog::Resize { w: self.doc.w, h: self.doc.h, anchor: (1, 1), scale: false };
                    }
                    if mi(ui, "Crop to Selection", "") {
                        self.crop_to_selection();
                    }
                    ui.separator();
                    if mi(ui, "Flip Canvas Horizontal", "") {
                        self.flip_canvas(true);
                    }
                    if mi(ui, "Flip Canvas Vertical", "") {
                        self.flip_canvas(false);
                    }
                    if mi(ui, "Rotate 90 CW", "") {
                        self.rotate_canvas(true);
                    }
                    if mi(ui, "Rotate 90 CCW", "") {
                        self.rotate_canvas(false);
                    }
                    if mi(ui, "Rotate Layer (RotSprite)...", "") {
                        self.dialog = Dialog::RotSprite { angle: 15.0, all: false };
                    }
                    ui.separator();
                    if mi(ui, "Flip Layer Horizontal", "") {
                        self.flip_layer(true);
                    }
                    if mi(ui, "Flip Layer Vertical", "") {
                        self.flip_layer(false);
                    }
                });
                ui.menu_button("Layer", |ui| {
                    if mi(ui, "New Layer", "") {
                        self.layer_add();
                    }
                    if mi(ui, "Duplicate Layer", "") {
                        self.layer_dup();
                    }
                    if mi(ui, "Delete Layer", "") {
                        self.layer_del();
                    }
                    ui.separator();
                    if mi(ui, "Move Up", "") {
                        self.layer_move(true);
                    }
                    if mi(ui, "Move Down", "") {
                        self.layer_move(false);
                    }
                    if mi(ui, "Merge Down", "") {
                        self.layer_merge();
                    }
                    if mi(ui, "Flatten Image", "") {
                        self.flatten();
                    }
                });
                ui.menu_button("Animation", |ui| {
                    if mi(ui, "New Frame", "") {
                        self.frame_add();
                    }
                    if mi(ui, "Duplicate Frame", "") {
                        self.frame_dup();
                    }
                    if mi(ui, "Delete Frame", "") {
                        self.frame_del();
                    }
                    ui.separator();
                    if mi(ui, "Reverse Frames", "") {
                        self.frame_reverse();
                    }
                    if mi(ui, "Add Ping-Pong Return", "") {
                        self.frame_pingpong();
                    }
                    ui.separator();
                    if mi(ui, "Rename Frame...", "") {
                        self.dialog = Dialog::RenameFrame(self.doc.names[self.cur_frame].clone());
                    }
                    if mi(ui, "New Tag...", "") {
                        self.dialog = start::dlg_new_tag(self);
                    }
                    if mi(ui, "Play All Frames (ignore tags)", "") {
                        self.play_tag = None;
                    }
                    ui.separator();
                    if mi(ui, "Play / Stop", "Enter") {
                        let c = ui.ctx().clone();
                        self.toggle_play(&c);
                    }
                    if mi(ui, "Previous Frame", ",") {
                        self.goto_frame(-1);
                    }
                    if mi(ui, "Next Frame", ".") {
                        self.goto_frame(1);
                    }
                });
                ui.menu_button("Filter", |ui| {
                    ui.label(RichText::new("Applies to current layer (selection-aware)").weak().small());
                    if mi(ui, "Invert Colors", "") {
                        self.filter_invert(false);
                    }
                    if mi(ui, "Grayscale", "") {
                        self.filter_gray(false);
                    }
                    if mi(ui, "Hue / Saturation / Value...", "") {
                        self.dialog = Dialog::Adjust { hue: 0.0, sat: 0.0, val: 0.0, all: false };
                    }
                    if mi(ui, "Outline, Shadow & Shading...", "") {
                        self.dialog = start::dlg_effects();
                    }
                    if mi(ui, "Replace Color (tolerance)...", "") {
                        self.dialog = Dialog::Replace { tol: 24, scope: 0 };
                    }
                    if mi(ui, "Replace Primary with Secondary", "") {
                        self.filter_replace(false);
                    }
                    if mi(ui, "Snap to Palette", "") {
                        self.filter_quantize(false);
                    }
                    ui.separator();
                    if mi(ui, "Invert Colors (all frames)", "") {
                        self.filter_invert(true);
                    }
                    if mi(ui, "Snap to Palette (all frames)", "") {
                        self.filter_quantize(true);
                    }
                });
                ui.menu_button("View", |ui| {
                    if mi(ui, "Zoom In", "+") {
                        self.zoom = (self.zoom + 1.0).min(64.0);
                    }
                    if mi(ui, "Zoom Out", "-") {
                        self.zoom = (self.zoom - 1.0).max(1.0);
                    }
                    if mi(ui, "Fit on Screen", "Ctrl+0") {
                        self.need_fit = true;
                    }
                    ui.separator();
                    ui.checkbox(&mut self.grid, "Pixel Grid");
                    ui.checkbox(&mut self.rulers, "Rulers");
                    ui.checkbox(&mut self.onion, "Onion Skin");
                    ui.checkbox(&mut self.tile_preview, "Tiled Preview");
                    ui.checkbox(&mut self.sym_x, "Mirror Horizontally");
                    ui.checkbox(&mut self.sym_y, "Mirror Vertically");
                    ui.separator();
                    let mut light = self.th.light;
                    if ui.selectable_label(!light, "Dark theme").clicked() {
                        light = false;
                    }
                    if ui.selectable_label(light, "Light theme").clicked() {
                        light = true;
                    }
                    if light != self.th.light {
                        let c = ui.ctx().clone();
                        self.set_light(&c, light);
                    }
                });
                ui.menu_button("Help", |ui| {
                    if mi(ui, "Keyboard Shortcuts", "") {
                        self.dialog = Dialog::Shortcuts;
                    }
                    if mi(ui, "About SpriteCraft", "") {
                        self.dialog = Dialog::About;
                    }
                });
                self.title_controls(ui);
            });
        });
    }

    // ------------------------------------------------------------ options bar
    fn options_bar(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::top("options").show(ctx, |ui| {
            ui.horizontal_wrapped(|ui| {
                let name = TOOLS.iter().find(|t| t.0 == self.tool).map(|t| t.1).unwrap_or("");
                ui.label(RichText::new(name).strong());
                ui.separator();
                let sized = matches!(self.tool, Tool::Pencil | Tool::Eraser | Tool::Line | Tool::Rect | Tool::Ellipse | Tool::Shade | Tool::Spray);
                if sized {
                    ui.add(egui::DragValue::new(&mut self.brush).range(1..=32).prefix("Size "));
                    ui.checkbox(&mut self.round, "Round");
                }
                match self.tool {
                    Tool::Pencil => {
                        ui.checkbox(&mut self.pixel_perfect, "Pixel-perfect");
                        ui.checkbox(&mut self.dither, "Dither");
                    }
                    Tool::Eraser | Tool::Spray => {
                        ui.checkbox(&mut self.dither, "Dither");
                        if self.tool == Tool::Spray {
                            ui.add(egui::DragValue::new(&mut self.spray_density).range(0.05..=1.0).speed(0.01).prefix("Density "));
                        }
                    }
                    Tool::Rect | Tool::Ellipse => {
                        ui.checkbox(&mut self.fill_shapes, "Filled");
                        ui.label(RichText::new("Shift = square / circle").weak().small());
                    }
                    Tool::Line => {
                        ui.label(RichText::new("Shift = snap angle").weak().small());
                    }
                    Tool::Bucket | Tool::Wand => {
                        ui.selectable_value(&mut self.global_fill, false, "Contiguous");
                        ui.selectable_value(&mut self.global_fill, true, "Global");
                        ui.add(egui::DragValue::new(&mut self.tolerance).range(0..=255).prefix("Tolerance "));
                    }
                    Tool::Shade => {
                        ui.selectable_value(&mut self.shade_darken, false, "Lighten");
                        ui.selectable_value(&mut self.shade_darken, true, "Darken");
                        ui.add(egui::DragValue::new(&mut self.shade_strength).range(0.02..=0.6).speed(0.01).prefix("Strength "));
                    }
                    Tool::Select | Tool::Move => {
                        ui.label(RichText::new("Shift = add to selection").weak().small());
                    }
                    Tool::Lasso => {
                        ui.label(RichText::new("Drag to draw a free-form selection. Shift = add to selection.").weak().small());
                    }
                    Tool::Picker => {
                        ui.label(RichText::new("Right-click = secondary color. Alt+click works with any tool.").weak().small());
                    }
                    Tool::Text => {
                        ui.add(egui::TextEdit::multiline(&mut self.text).desired_rows(1).desired_width(170.0).hint_text("Type text..."));
                        egui::ComboBox::from_id_salt("fontsel").selected_text(self.text_font_name.clone()).width(150.0).show_ui(ui, |ui| {
                            ScrollArea::vertical().max_height(320.0).show(ui, |ui| {
                                for (name, _) in &self.fonts {
                                    if ui.selectable_label(*name == self.text_font_name, name).clicked() {
                                        self.text_font_name = name.clone();
                                    }
                                }
                            });
                        });
                        ui.add(egui::DragValue::new(&mut self.text_size).range(4.0..=128.0).speed(0.25).prefix("Size "));
                        ui.checkbox(&mut self.text_smooth, "Smooth");
                        if !self.text_smooth {
                            ui.add(egui::DragValue::new(&mut self.text_thresh).range(0.1..=0.9).speed(0.01).prefix("Pixelate "));
                        }
                        ui.label(RichText::new("Click canvas to place (primary color)").weak().small());
                    }
                    Tool::Hand => {}
                }
                ui.separator();
                ui.toggle_value(&mut self.sym_x, "Mirror X");
                ui.toggle_value(&mut self.sym_y, "Mirror Y");
                ui.toggle_value(&mut self.grid, "Grid");
                ui.toggle_value(&mut self.onion, "Onion");
            });
        });
    }

    fn status_bar(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::bottom("status").exact_height(22.0).show(ctx, |ui| {
            ui.horizontal(|ui| {
                let pos = self.hover.map(|(x, y)| format!("{x}, {y}")).unwrap_or_else(|| "-".into());
                ui.label(RichText::new(format!("X,Y {pos}")).monospace().small());
                ui.separator();
                ui.label(RichText::new(format!("{}x{} px", self.doc.w, self.doc.h)).monospace().small());
                ui.separator();
                ui.label(RichText::new(format!("{}%", (self.zoom * 100.0) as i32)).monospace().small());
                ui.separator();
                ui.label(RichText::new(format!("Frame {}/{}  Layer {}/{}", self.cur_frame + 1, self.doc.frames.len(), self.cur_layer + 1, self.doc.layers.len())).small());
                ui.separator();
                ui.label(RichText::new(&self.status).small().weak());
            });
        });
    }

    // ------------------------------------------------------------ toolbox
    fn toolbox(&mut self, ctx: &egui::Context) {
        egui::SidePanel::left("tools").exact_width(48.0).resizable(false).show(ctx, |ui| {
            ui.vertical_centered(|ui| {
                ui.add_space(4.0);
                for &(t, name, key) in TOOLS {
                    let (rect, resp) = ui.allocate_exact_size(vec2(34.0, 34.0), Sense::click());
                    let sel = self.tool == t;
                    let bg = if sel { ACCENT } else if resp.hovered() { self.th.btn_hover } else { Color32::TRANSPARENT };
                    ui.painter().rect_filled(rect, 6.0, bg);
                    draw_icon(ui.painter(), rect, t, if sel { Color32::WHITE } else { self.th.text });
                    if resp.on_hover_text(format!("{name} ({key})")).clicked() {
                        self.tool = t;
                    }
                }
                ui.add_space(10.0);
                ui.separator();
                let (rect, resp) = ui.allocate_exact_size(vec2(38.0, 38.0), Sense::click());
                let p = ui.painter();
                let bgr = Rect::from_min_size(rect.min + vec2(13.0, 13.0), vec2(24.0, 24.0));
                let fgr = Rect::from_min_size(rect.min, vec2(24.0, 24.0));
                p.rect_filled(bgr, 3.0, self.color2);
                p.rect_stroke(bgr, 3.0, Stroke::new(1.5, Color32::from_gray(200)));
                p.rect_filled(fgr, 3.0, self.color);
                p.rect_stroke(fgr, 3.0, Stroke::new(1.5, Color32::WHITE));
                if resp.on_hover_text("Primary / secondary color (click or X to swap)").clicked() {
                    std::mem::swap(&mut self.color, &mut self.color2);
                }
            });
        });
    }

    // ------------------------------------------------------------ right panels
    fn right_panel(&mut self, ctx: &egui::Context) {
        egui::SidePanel::right("panels").default_width(272.0).min_width(240.0).show(ctx, |ui| {
            ScrollArea::vertical().show(ui, |ui| {
                egui::CollapsingHeader::new(RichText::new("Preview").strong()).default_open(true).show(ui, |ui| self.preview_ui(ui));
                egui::CollapsingHeader::new(RichText::new("Color").strong()).default_open(true).show(ui, |ui| self.color_ui(ui));
                egui::CollapsingHeader::new(RichText::new("Swatches").strong()).default_open(true).show(ui, |ui| self.swatches_ui(ui));
                egui::CollapsingHeader::new(RichText::new("Project colors").strong()).default_open(false).show(ui, |ui| self.project_colors_ui(ui));
                egui::CollapsingHeader::new(RichText::new("Layers").strong()).default_open(true).show(ui, |ui| self.layers_ui(ui));
            });
        });
    }

    fn preview_ui(&mut self, ui: &mut Ui) {
        let Some(tex) = self.prev_tex.clone() else { return };
        let (w, h) = (self.doc.w as f32, self.doc.h as f32);
        let tiles = if self.tile_preview { 3.0 } else { 1.0 };
        let mut s = (ui.available_width() / (w * tiles)).min(150.0 / (h * tiles));
        if s >= 1.0 {
            s = s.floor();
        }
        let size = vec2(w * tiles * s, h * tiles * s);
        let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), size.y.max(40.0)), Sense::hover());
        let p = ui.painter_at(rect);
        p.rect_filled(rect, 4.0, self.th.well);
        let origin = rect.center() - size / 2.0;
        for ty in 0..tiles as i32 {
            for tx in 0..tiles as i32 {
                let r = Rect::from_min_size(origin + vec2(tx as f32 * w * s, ty as f32 * h * s), vec2(w * s, h * s));
                p.image(tex.id(), r, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE);
            }
        }
        ui.horizontal(|ui| {
            let label = if self.playing { "Stop" } else { "Play" };
            if ui.button(label).clicked() {
                let c = ui.ctx().clone();
                self.toggle_play(&c);
            }
            ui.add(egui::DragValue::new(&mut self.doc.fps).range(1..=60).suffix(" fps"));
            ui.checkbox(&mut self.tile_preview, "Tile");
        });
    }

    fn color_ui(&mut self, ui: &mut Ui) {
        ui.spacing_mut().slider_width = 200.0;
        let mut c = self.color;
        if egui::color_picker::color_picker_color32(ui, &mut c, egui::color_picker::Alpha::OnlyBlend) {
            self.color = c;
        }
        ui.horizontal(|ui| {
            ui.label("Hex #");
            let r = ui.add(egui::TextEdit::singleline(&mut self.hex_text).desired_width(70.0));
            if r.changed() {
                let t = self.hex_text.trim().trim_start_matches('#');
                if t.len() == 6 {
                    if let Ok(v) = u32::from_str_radix(t, 16) {
                        self.color = hex_color(v);
                    }
                }
            }
            if !r.has_focus() {
                let [cr, cg, cb, _] = px_of(self.color);
                self.hex_text = format!("{cr:02X}{cg:02X}{cb:02X}");
            }
            if ui.button("Swap").clicked() {
                std::mem::swap(&mut self.color, &mut self.color2);
            }
        });
    }

    fn swatches_ui(&mut self, ui: &mut Ui) {
        ui.horizontal(|ui| {
            egui::ComboBox::from_id_salt("preset").selected_text("Load preset").show_ui(ui, |ui| {
                for pr in palettes::PRESETS {
                    if ui.selectable_label(false, format!("{} ({})", pr.name, pr.colors.len())).clicked() {
                        self.palette = pr.colors.iter().map(|&c| hex_color(c)).collect();
                    }
                }
            });
            if ui.button("+").on_hover_text("Add primary color to palette").clicked() && !self.palette.contains(&self.color) {
                self.palette.push(self.color);
            }
            ui.toggle_value(&mut self.palette_remove, "-").on_hover_text("Click swatches to remove them");
            if ui.button("Extract").on_hover_text("Build palette from the current frame").clicked() {
                self.extract_palette();
            }
        });
        let mut remove = None;
        ui.scope(|ui| {
            ui.spacing_mut().item_spacing = vec2(2.0, 2.0);
            ui.horizontal_wrapped(|ui| {
                for (i, &c) in self.palette.iter().enumerate() {
                    let (rect, resp) = ui.allocate_exact_size(vec2(18.0, 18.0), Sense::click());
                    ui.painter().rect_filled(rect, 2.0, c);
                    let sel = c == self.color;
                    ui.painter().rect_stroke(rect, 2.0, Stroke::new(if sel { 2.0 } else { 1.0 }, if sel { self.th.strong } else { Color32::from_black_alpha(120) }));
                    let [r, g, b, _] = px_of(c);
                    let resp = resp.on_hover_text(format!("#{r:02X}{g:02X}{b:02X}  (right-click: secondary)"));
                    if resp.clicked() {
                        if self.palette_remove {
                            remove = Some(i);
                        } else {
                            self.color = c;
                        }
                    }
                    if resp.secondary_clicked() {
                        self.color2 = c;
                    }
                }
            });
        });
        if let Some(i) = remove {
            self.palette.remove(i);
        }
    }

    fn project_colors_ui(&mut self, ui: &mut Ui) {
        if self.colors_dirty && self.stroke.is_none() {
            self.refresh_project_colors();
        }
        ui.label(RichText::new(format!("{} colors in this project", self.project_colors.len())).small().color(self.th.dim));
        ui.label(RichText::new("Click: primary. Right-click for more (select pixels, replace, add to palette).").small().weak());
        let mut act: Option<(Px, u8)> = None;
        ui.scope(|ui| {
            ui.spacing_mut().item_spacing = vec2(2.0, 2.0);
            ui.horizontal_wrapped(|ui| {
                for (c, n) in self.project_colors.iter().take(300) {
                    let (rect, resp) = ui.allocate_exact_size(vec2(16.0, 16.0), Sense::click());
                    ui.painter().rect_filled(rect, 2.0, color_of(*c));
                    ui.painter().rect_stroke(rect, 2.0, Stroke::new(1.0, Color32::from_black_alpha(110)));
                    let resp = resp.on_hover_text(format!("#{:02X}{:02X}{:02X}  -  {} px", c[0], c[1], c[2], n));
                    if resp.clicked() {
                        act = Some((*c, 0));
                    }
                    resp.context_menu(|ui| {
                        for (label, code) in [("Set as primary", 0u8), ("Set as secondary", 1), ("Select these pixels (layer)", 2), ("Select these pixels (flattened frame)", 3), ("Add to palette", 4), ("Replace with secondary (layer)", 5)] {
                            if ui.button(label).clicked() {
                                act = Some((*c, code));
                                ui.close_menu();
                            }
                        }
                    });
                }
            });
        });
        if let Some((c, code)) = act {
            match code {
                0 => self.color = color_of(c),
                1 => self.color2 = color_of(c),
                2 => self.select_color(c, 0, false),
                3 => self.select_color(c, 0, true),
                4 => {
                    if !self.palette.contains(&color_of(c)) {
                        self.palette.push(color_of(c));
                    }
                }
                _ => {
                    let (a, b) = (self.color, self.color2);
                    self.color = color_of(c);
                    self.fx_replace(0, 0);
                    self.color = a;
                    self.color2 = b;
                }
            }
        }
    }

    fn layers_ui(&mut self, ui: &mut Ui) {
        let n = self.doc.layers.len();
        for idx in (0..n).rev() {
            ui.horizontal(|ui| {
                let (r, resp) = ui.allocate_exact_size(vec2(22.0, 22.0), Sense::click());
                let vis = self.doc.layers[idx].visible;
                eye_icon(ui.painter(), r, vis, if vis { self.th.strong } else { Color32::GRAY });
                if resp.on_hover_text("Toggle visibility").clicked() {
                    self.doc.layers[idx].visible = !vis;
                    self.changed_all();
                }
                let (r, resp) = ui.allocate_exact_size(vec2(22.0, 22.0), Sense::click());
                let lk = self.doc.layers[idx].locked;
                lock_icon(ui.painter(), r, lk, if lk { Color32::from_rgb(240, 180, 60) } else { Color32::GRAY });
                if resp.on_hover_text("Lock layer").clicked() {
                    self.doc.layers[idx].locked = !lk;
                }
                if self.rename_layer == Some(idx) {
                    let r = ui.add(egui::TextEdit::singleline(&mut self.doc.layers[idx].name).desired_width(120.0));
                    r.request_focus();
                    if r.lost_focus() {
                        self.rename_layer = None;
                    }
                } else {
                    let name = self.doc.layers[idx].name.clone();
                    let resp = ui.add_sized([ui.available_width(), 22.0], egui::SelectableLabel::new(idx == self.cur_layer, name));
                    if resp.clicked() {
                        self.cur_layer = idx;
                    }
                    if resp.double_clicked() {
                        self.rename_layer = Some(idx);
                    }
                }
            });
        }
        let mut op = self.doc.layers[self.cur_layer].opacity;
        let resp = ui.add(egui::Slider::new(&mut op, 0.0..=1.0).text("Opacity"));
        if resp.drag_started() {
            self.push_undo();
        }
        if resp.changed() {
            self.doc.layers[self.cur_layer].opacity = op;
            self.changed_all();
        }
        ui.horizontal_wrapped(|ui| {
            if ui.button("New").clicked() {
                self.layer_add();
            }
            if ui.button("Dup").clicked() {
                self.layer_dup();
            }
            if ui.button("Del").clicked() {
                self.layer_del();
            }
            if ui.button("Up").clicked() {
                self.layer_move(true);
            }
            if ui.button("Down").clicked() {
                self.layer_move(false);
            }
            if ui.button("Merge").clicked() {
                self.layer_merge();
            }
        });
    }

    // ------------------------------------------------------------ timeline
    fn timeline(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::bottom("timeline").resizable(true).default_height(150.0).min_height(110.0).show(ctx, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.label(RichText::new("Timeline").strong());
                ui.separator();
                if ui.button("Prev").clicked() {
                    self.goto_frame(-1);
                }
                let label = if self.playing { "Stop" } else { "Play" };
                if ui.add(egui::Button::new(label).fill(if self.playing { ACCENT } else { ui.visuals().widgets.inactive.bg_fill })).clicked() {
                    let c = ui.ctx().clone();
                    self.toggle_play(&c);
                }
                if ui.button("Next").clicked() {
                    self.goto_frame(1);
                }
                ui.separator();
                if ui.button("New").clicked() {
                    self.frame_add();
                }
                if ui.button("Dup").clicked() {
                    self.frame_dup();
                }
                if ui.button("Del").clicked() {
                    self.frame_del();
                }
                if ui.button("<").on_hover_text("Move frame left").clicked() {
                    self.frame_move(false);
                }
                if ui.button(">").on_hover_text("Move frame right").clicked() {
                    self.frame_move(true);
                }
                ui.separator();
                let f = self.cur_frame;
                ui.add(egui::DragValue::new(&mut self.doc.holds[f]).range(1..=32).prefix("Hold x"));
                ui.add(egui::DragValue::new(&mut self.doc.fps).range(1..=60).suffix(" fps"));
                ui.separator();
                ui.checkbox(&mut self.onion, "Onion skin");
                if self.onion {
                    ui.add(egui::DragValue::new(&mut self.onion_n).range(1..=4).prefix("+/- "));
                }
            });
            ui.separator();
            ScrollArea::horizontal().auto_shrink([false, false]).show(ui, |ui| {
                let strip_y = ui.cursor().min.y;
                ui.add_space(20.0);
                let mut cell_rects: Vec<Rect> = vec![];
                ui.horizontal(|ui| {
                    let (w, h) = (self.doc.w as f32, self.doc.h as f32);
                    for i in 0..self.doc.frames.len() {
                        let (rect, resp) = ui.allocate_exact_size(vec2(66.0, 74.0), Sense::click());
                        cell_rects.push(rect);
                        let sel = i == self.cur_frame;
                        let p = ui.painter();
                        p.rect_filled(rect, 4.0, if sel { Color32::from_rgb(30, 80, 150) } else { self.th.cell });
                        let area = Rect::from_min_size(rect.min + vec2(5.0, 4.0), vec2(56.0, 50.0));
                        p.rect_filled(area, 2.0, self.th.well);
                        let s = (area.width() / w).min(area.height() / h);
                        let ir = Rect::from_center_size(area.center(), vec2(w * s, h * s));
                        if let Some(Some(t)) = self.thumbs.get(i) {
                            p.image(t.id(), ir, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE);
                        }
                        let hold = self.doc.holds[i];
                        let nm: String = self.doc.names[i].chars().take(7).collect();
                        let txt = match (hold > 1, nm.is_empty()) {
                            (true, true) => format!("{}  x{}", i + 1, hold),
                            (true, false) => format!("{} {} x{}", i + 1, nm, hold),
                            (false, true) => format!("{}", i + 1),
                            (false, false) => format!("{} {}", i + 1, nm),
                        };
                        p.text(pos2(rect.center().x, rect.bottom() - 9.0), Align2::CENTER_CENTER, txt, FontId::proportional(11.0), if sel { Color32::WHITE } else { self.th.text });
                        if self.playing && i == self.play_frame {
                            p.rect_stroke(rect, 4.0, Stroke::new(2.0, Color32::from_rgb(120, 220, 140)));
                        } else if sel {
                            p.rect_stroke(rect, 4.0, Stroke::new(2.0, ACCENT));
                        }
                        if resp.clicked() || resp.secondary_clicked() {
                            self.cur_frame = i;
                            self.dirty = true;
                        }
                        resp.context_menu(|ui| {
                            if ui.button("New Frame").clicked() {
                                self.frame_add();
                                ui.close_menu();
                            }
                            if ui.button("Insert Frame Before").clicked() {
                                self.frame_insert_before();
                                ui.close_menu();
                            }
                            if ui.button("Duplicate Frame").clicked() {
                                self.frame_dup();
                                ui.close_menu();
                            }
                            if ui.button("Delete Frame").clicked() {
                                self.frame_del();
                                ui.close_menu();
                            }
                            if ui.button("Clear Frame").clicked() {
                                self.frame_clear();
                                ui.close_menu();
                            }
                            ui.separator();
                            if ui.button("Move Left").clicked() {
                                self.frame_move(false);
                                ui.close_menu();
                            }
                            if ui.button("Move Right").clicked() {
                                self.frame_move(true);
                                ui.close_menu();
                            }
                            ui.separator();
                            if ui.button("Hold +1").clicked() {
                                self.hold_adjust(1);
                            }
                            if ui.button("Hold -1").clicked() {
                                self.hold_adjust(-1);
                            }
                            ui.separator();
                            if ui.button("Rename Frame...").clicked() {
                                self.dialog = Dialog::RenameFrame(self.doc.names[self.cur_frame].clone());
                                ui.close_menu();
                            }
                            if ui.button("New Tag Starting Here...").clicked() {
                                self.dialog = start::dlg_new_tag(self);
                                ui.close_menu();
                            }
                            ui.separator();
                            if ui.button("Reverse All Frames").clicked() {
                                self.frame_reverse();
                                ui.close_menu();
                            }
                            if ui.button("Add Ping-Pong Return").clicked() {
                                self.frame_pingpong();
                                ui.close_menu();
                            }
                            if ui.button("Play From Here").clicked() {
                                let c = ui.ctx().clone();
                                self.playing = false;
                                self.toggle_play(&c);
                                ui.close_menu();
                            }
                        });
                    }
                });
                self.draw_tags(ui, &cell_rects, strip_y);
            });
        });
    }

    fn draw_tags(&mut self, ui: &mut Ui, cells: &[Rect], strip_y: f32) {
        let tags = self.doc.tags.clone();
        for (ti, t) in tags.iter().enumerate() {
            let (Some(r0), Some(r1)) = (cells.get(t.from), cells.get(t.to)) else { continue };
            let rect = Rect::from_min_max(pos2(r0.min.x, strip_y + 2.0), pos2(r1.max.x, strip_y + 18.0));
            let resp = ui.interact(rect, egui::Id::new(("tag", ti)), Sense::click());
            let col = App::tag_color(t);
            let p = ui.painter();
            p.rect_filled(rect, 3.0, col.gamma_multiply(if resp.hovered() { 1.0 } else { 0.8 }));
            if self.play_tag == Some(ti) {
                p.rect_stroke(rect, 3.0, Stroke::new(2.0, self.th.strong));
            }
            let arrow = match t.dir {
                TagDir::Forward => "",
                TagDir::Reverse => " <",
                TagDir::PingPong => " <>",
            };
            ui.painter_at(rect).text(rect.left_center() + vec2(6.0, 0.0), Align2::LEFT_CENTER, format!("{}{}", t.name, arrow), FontId::proportional(11.5), Color32::WHITE);
            let resp = resp.on_hover_text("Click: play only this tag. Double-click: edit. Right-click: more.");
            if resp.clicked() {
                self.play_tag = if self.play_tag == Some(ti) { None } else { Some(ti) };
                self.status = match self.play_tag {
                    Some(_) => format!("Playback limited to tag \"{}\"", t.name),
                    None => "Playing all frames".into(),
                };
            }
            if resp.double_clicked() {
                self.dialog = Dialog::Tag { idx: Some(ti), name: t.name.clone(), from: t.from, to: t.to, dir: t.dir as usize, color: t.color };
            }
            resp.context_menu(|ui| {
                if ui.button("Edit Tag...").clicked() {
                    self.dialog = Dialog::Tag { idx: Some(ti), name: t.name.clone(), from: t.from, to: t.to, dir: t.dir as usize, color: t.color };
                    ui.close_menu();
                }
                if ui.button("Delete Tag").clicked() {
                    self.tag_delete(ti);
                    ui.close_menu();
                }
                if ui.button("Play Only This Tag").clicked() {
                    self.play_tag = Some(ti);
                    ui.close_menu();
                }
                if ui.button("Play All Frames").clicked() {
                    self.play_tag = None;
                    ui.close_menu();
                }
            });
        }
    }

    // ------------------------------------------------------------ canvas
    fn canvas_panel(&mut self, ctx: &egui::Context) {
        egui::CentralPanel::default().frame(Frame::none().fill(self.th.workspace)).show(ctx, |ui| {
            self.tab_bar(ui);
            let (w, h) = (self.doc.w as f32, self.doc.h as f32);
            let full = ui.available_rect_before_wrap();
            let ruler = if self.rulers { 18.0 } else { 0.0 };
            let area = Rect::from_min_max(full.min + vec2(ruler, ruler), full.max);
            let resp = ui.allocate_rect(area, Sense::click_and_drag());

            if self.need_fit && area.width() > 50.0 {
                self.zoom = (area.width() * 0.9 / w).min(area.height() * 0.9 / h).floor().clamp(1.0, 64.0);
                self.pan = vec2(0.0, 0.0);
                self.need_fit = false;
            }

            let (pos, p0, p1, d0, d1, d2, delta, alt, space, raw_scroll, zoomd) = ctx.input(|i| {
                (
                    i.pointer.latest_pos(),
                    i.pointer.button_pressed(PointerButton::Primary),
                    i.pointer.button_pressed(PointerButton::Secondary),
                    i.pointer.button_down(PointerButton::Primary),
                    i.pointer.button_down(PointerButton::Secondary),
                    i.pointer.button_down(PointerButton::Middle),
                    i.pointer.delta(),
                    i.modifiers.alt,
                    i.key_down(Key::Space),
                    i.raw_scroll_delta.y,
                    i.zoom_delta(),
                )
            });
            let hovered = resp.hovered();
            let origin_of = |z: f32, pan: egui::Vec2| area.center() + pan - vec2(w, h) * z / 2.0;

            // zoom around cursor
            if hovered && (raw_scroll != 0.0 || zoomd != 1.0) {
                if let Some(pp) = pos {
                    let old = self.zoom;
                    let mut nz = old;
                    if raw_scroll > 0.5 {
                        nz = (old * 1.2).ceil();
                    } else if raw_scroll < -0.5 {
                        nz = (old / 1.2).floor();
                    }
                    if zoomd != 1.0 {
                        nz = (old * zoomd).round();
                    }
                    let nz = nz.clamp(1.0, 64.0);
                    if nz != old {
                        let f = (pp - origin_of(old, self.pan)) / old;
                        let neworg = pp - f * nz;
                        self.zoom = nz;
                        self.pan = neworg + vec2(w, h) * nz / 2.0 - area.center();
                    }
                }
            }

            // panning
            let hand = space || self.tool == Tool::Hand;
            if (d2 && (hovered || self.panning)) || (hand && d0 && (hovered || self.panning)) {
                self.pan += delta;
                self.panning = true;
            } else {
                self.panning = false;
            }

            let z = self.zoom;
            let org = origin_of(z, self.pan);
            let to_px = |p: Pos2| -> (i32, i32) {
                let v = (p - org) / z;
                (v.x.floor() as i32, v.y.floor() as i32)
            };
            self.hover = if hovered { pos.map(to_px) } else { None };
            if hovered {
                ctx.set_cursor_icon(if self.panning || hand { egui::CursorIcon::Grab } else { egui::CursorIcon::Crosshair });
            }

            // drawing input
            if !self.panning && !self.playing {
                if self.stroke.is_some() {
                    let btn = self.stroke.as_ref().map(|s| s.button).unwrap_or(0);
                    let down = if btn == 0 { d0 } else { d1 };
                    if down {
                        if let Some(pp) = pos {
                            self.drag_stroke(to_px(pp));
                        }
                    } else {
                        self.end_stroke();
                    }
                } else if hovered && !hand {
                    if let Some(pp) = pos {
                        if p0 {
                            self.begin_stroke(to_px(pp), 0, alt);
                        } else if p1 {
                            self.begin_stroke(to_px(pp), 1, alt);
                        }
                    }
                }
            }

            // texture for the main view
            let img = self.build_main_image();
            match &mut self.main_tex {
                Some(t) => t.set(img, TextureOptions::NEAREST),
                None => self.main_tex = Some(ctx.load_texture("main", img, TextureOptions::NEAREST)),
            }

            if self.tool == Tool::Text && self.hover.is_some() {
                self.ensure_text();
                if let Some(c) = &self.text_cache {
                    let img = to_color_image(c.w, c.h, &c.px);
                    match &mut self.text_tex {
                        Some((k, t)) => {
                            if *k != c.key {
                                t.set(img, TextureOptions::NEAREST);
                                *k = c.key.clone();
                            }
                        }
                        None => self.text_tex = Some((c.key.clone(), ctx.load_texture("textghost", img, TextureOptions::NEAREST))),
                    }
                }
            }

            // ---- paint
            let painter = ui.painter_at(area);
            let size = vec2(w, h) * z;
            let crect = Rect::from_min_size(org, size);
            painter.rect_filled(crect.translate(vec2(4.0, 4.0)), 0.0, Color32::from_black_alpha(70));
            let vis = crect.intersect(area);
            if vis.is_positive() {
                painter.rect_filled(vis, 0.0, Color32::from_gray(if self.th.light { 196 } else { 58 }));
                let sq = 12.0;
                let (i0, i1) = (((vis.min.x - org.x) / sq).floor() as i32, ((vis.max.x - org.x) / sq).ceil() as i32);
                let (j0, j1) = (((vis.min.y - org.y) / sq).floor() as i32, ((vis.max.y - org.y) / sq).ceil() as i32);
                for j in j0..=j1 {
                    for i in i0..=i1 {
                        if (i + j) & 1 == 0 {
                            continue;
                        }
                        let r = Rect::from_min_size(org + vec2(i as f32 * sq, j as f32 * sq), vec2(sq, sq)).intersect(vis);
                        if r.is_positive() {
                            painter.rect_filled(r, 0.0, Color32::from_gray(if self.th.light { 222 } else { 72 }));
                        }
                    }
                }
            }
            if let Some(t) = &self.main_tex {
                painter.image(t.id(), crect, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE);
            }
            if self.grid && z >= 6.0 {
                let gc = if self.th.light { Color32::from_rgba_unmultiplied(0, 0, 0, 22) } else { Color32::from_rgba_unmultiplied(255, 255, 255, 16) };
                for x in 0..=self.doc.w {
                    let sx = org.x + x as f32 * z;
                    painter.line_segment([pos2(sx, org.y), pos2(sx, org.y + size.y)], Stroke::new(1.0, gc));
                }
                for y in 0..=self.doc.h {
                    let sy = org.y + y as f32 * z;
                    painter.line_segment([pos2(org.x, sy), pos2(org.x + size.x, sy)], Stroke::new(1.0, gc));
                }
            }
            painter.rect_stroke(crect, 0.0, Stroke::new(1.0, Color32::from_gray(20)));
            let guide = Stroke::new(1.0, Color32::from_rgb(255, 80, 160));
            if self.sym_x {
                painter.line_segment([pos2(org.x + size.x / 2.0, org.y - 6.0), pos2(org.x + size.x / 2.0, org.y + size.y + 6.0)], guide);
            }
            if self.sym_y {
                painter.line_segment([pos2(org.x - 6.0, org.y + size.y / 2.0), pos2(org.x + size.x + 6.0, org.y + size.y / 2.0)], guide);
            }
            // selection outline
            if let Some(sel) = &self.selection {
                let (iw, ih) = (self.doc.w, self.doc.h);
                let mut segs: Vec<[Pos2; 2]> = vec![];
                for i in 0..iw * ih {
                    if !sel[i] {
                        continue;
                    }
                    let (x, y) = (i % iw, i / iw);
                    let a = org + vec2(x as f32, y as f32) * z;
                    if y == 0 || !sel[i - iw] {
                        segs.push([a, a + vec2(z, 0.0)]);
                    }
                    if y + 1 == ih || !sel[i + iw] {
                        segs.push([a + vec2(0.0, z), a + vec2(z, z)]);
                    }
                    if x == 0 || !sel[i - 1] {
                        segs.push([a, a + vec2(0.0, z)]);
                    }
                    if x + 1 == iw || !sel[i + 1] {
                        segs.push([a + vec2(z, 0.0), a + vec2(z, z)]);
                    }
                }
                for s in &segs {
                    painter.line_segment(*s, Stroke::new(3.0, Color32::BLACK));
                }
                for s in &segs {
                    painter.line_segment(*s, Stroke::new(1.5, Color32::WHITE));
                }
            }
            if let (Tool::Text, Some((hx, hy)), Some(c), Some((_, t))) = (self.tool, self.hover, &self.text_cache, &self.text_tex) {
                let r = Rect::from_min_size(org + vec2(hx as f32, hy as f32) * z, vec2(c.w as f32, c.h as f32) * z);
                painter.image(t.id(), r, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::from_white_alpha(170));
                painter.rect_stroke(r, 0.0, Stroke::new(1.0, Color32::from_rgba_unmultiplied(255, 255, 255, 90)));
            }
            if let Some(st) = &self.stroke {
                if st.tool == Tool::Lasso && st.pts.len() > 1 {
                    let path: Vec<Pos2> = st.pts.iter().map(|&(x, y)| org + vec2(x as f32 + 0.5, y as f32 + 0.5) * z).collect();
                    painter.add(egui::Shape::line(path.clone(), Stroke::new(3.0, Color32::BLACK)));
                    painter.add(egui::Shape::line(path, Stroke::new(1.5, Color32::WHITE)));
                }
            }
            // brush cursor
            if let (Some((hx, hy)), false) = (self.hover, self.panning || hand) {
                let sized = matches!(self.tool, Tool::Pencil | Tool::Eraser | Tool::Shade | Tool::Spray | Tool::Line | Tool::Rect | Tool::Ellipse);
                let s = if sized { self.brush.max(1) } else { 1 };
                let o = (s - 1) / 2;
                let r = Rect::from_min_size(org + vec2((hx - o) as f32, (hy - o) as f32) * z, vec2(s as f32, s as f32) * z);
                painter.rect_stroke(r, 0.0, Stroke::new(2.0, Color32::BLACK));
                painter.rect_stroke(r.shrink(1.0), 0.0, Stroke::new(1.0, Color32::WHITE));
            }

            // rulers
            if self.rulers {
                let rp = ui.painter_at(full);
                let rc = self.th.ruler;
                let tc = self.th.dim;
                rp.rect_filled(Rect::from_min_max(full.min, pos2(full.max.x, full.min.y + ruler)), 0.0, rc);
                rp.rect_filled(Rect::from_min_max(full.min, pos2(full.min.x + ruler, full.max.y)), 0.0, rc);
                let mut step = 1usize;
                while (step as f32) * z < 36.0 {
                    step *= 2;
                }
                let font = FontId::monospace(9.0);
                for x in (0..=self.doc.w).step_by(step) {
                    let sx = org.x + x as f32 * z;
                    if sx >= area.min.x && sx <= area.max.x {
                        rp.line_segment([pos2(sx, full.min.y + 10.0), pos2(sx, full.min.y + ruler)], Stroke::new(1.0, tc));
                        rp.text(pos2(sx + 2.0, full.min.y + 1.0), Align2::LEFT_TOP, x.to_string(), font.clone(), tc);
                    }
                }
                for y in (0..=self.doc.h).step_by(step) {
                    let sy = org.y + y as f32 * z;
                    if sy >= area.min.y && sy <= area.max.y {
                        rp.line_segment([pos2(full.min.x + 10.0, sy), pos2(full.min.x + ruler, sy)], Stroke::new(1.0, tc));
                        rp.text(pos2(full.min.x + 1.0, sy + 1.0), Align2::LEFT_TOP, y.to_string(), font.clone(), tc);
                    }
                }
                if let (Some(pp), true) = (pos, hovered) {
                    rp.line_segment([pos2(pp.x, full.min.y), pos2(pp.x, full.min.y + ruler)], Stroke::new(1.0, ACCENT));
                    rp.line_segment([pos2(full.min.x, pp.y), pos2(full.min.x + ruler, pp.y)], Stroke::new(1.0, ACCENT));
                }
            }
        });
    }

    // ------------------------------------------------------------ window chrome (Windows-style)
    fn title_controls(&mut self, ui: &mut Ui) {
        let ctx = ui.ctx().clone();
        let maxed = ctx.input(|i| i.viewport().maximized.unwrap_or(false));
        let (name, modified) = self.tab_title(self.active);
        let th = self.th;
        let home_only = self.placeholder_only();
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.spacing_mut().item_spacing.x = 0.0;
            if caption_button(ui, 3) {
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
            if caption_button(ui, if maxed { 2 } else { 1 }) {
                ctx.send_viewport_cmd(egui::ViewportCommand::Maximized(!maxed));
            }
            if caption_button(ui, 0) {
                ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(true));
            }
            let w = ui.available_width();
            let (rect, resp) = ui.allocate_exact_size(vec2(w, 32.0), Sense::click_and_drag());
            ui.painter_at(rect).text(rect.center(), Align2::CENTER_CENTER, if home_only { "SpriteCraft".to_string() } else { format!("{name}{} - SpriteCraft", if modified { " *" } else { "" }) }, FontId::proportional(12.0), th.dim);
            if resp.double_clicked() {
                ctx.send_viewport_cmd(egui::ViewportCommand::Maximized(!maxed));
            } else if resp.drag_started() {
                ctx.send_viewport_cmd(egui::ViewportCommand::StartDrag);
            }
        });
    }

    fn window_chrome(&mut self, ctx: &egui::Context) {
        let maxed = ctx.input(|i| i.viewport().maximized.unwrap_or(false));
        let s = ctx.screen_rect();
        if !maxed {
            ctx.layer_painter(egui::LayerId::new(egui::Order::Foreground, egui::Id::new("winborder")))
                .rect_stroke(s.shrink(0.5), 0.0, Stroke::new(1.0, self.th.border));
            use egui::{CursorIcon as C, ResizeDirection as D};
            let (g, c) = (5.0, 14.0);
            let regions = [
                (Rect::from_min_max(s.min + vec2(c, 0.0), pos2(s.max.x - c, s.min.y + g)), D::North, C::ResizeNorth),
                (Rect::from_min_max(pos2(s.min.x + c, s.max.y - g), s.max - vec2(c, 0.0)), D::South, C::ResizeSouth),
                (Rect::from_min_max(s.min + vec2(0.0, c), pos2(s.min.x + g, s.max.y - c)), D::West, C::ResizeWest),
                (Rect::from_min_max(pos2(s.max.x - g, s.min.y + c), s.max - vec2(0.0, c)), D::East, C::ResizeEast),
                (Rect::from_min_size(s.min, vec2(c, c)), D::NorthWest, C::ResizeNorthWest),
                (Rect::from_min_size(pos2(s.max.x - c, s.min.y), vec2(c, c)), D::NorthEast, C::ResizeNorthEast),
                (Rect::from_min_size(pos2(s.min.x, s.max.y - c), vec2(c, c)), D::SouthWest, C::ResizeSouthWest),
                (Rect::from_min_size(s.max - vec2(c, c), vec2(c, c)), D::SouthEast, C::ResizeSouthEast),
            ];
            for (i, (r, dir, cur)) in regions.into_iter().enumerate() {
                egui::Area::new(egui::Id::new(("resize", i))).order(egui::Order::Foreground).fixed_pos(r.min).show(ctx, |ui| {
                    let (_, resp) = ui.allocate_exact_size(r.size(), Sense::drag());
                    if resp.hovered() || resp.dragged() {
                        ctx.set_cursor_icon(cur);
                    }
                    if resp.drag_started() {
                        ctx.send_viewport_cmd(egui::ViewportCommand::BeginResize(dir));
                    }
                });
            }
        }
    }

    // ------------------------------------------------------------ project tabs
    pub fn request_close(&mut self, i: usize) {
        if self.tab_title(i).1 {
            self.dialog = Dialog::ConfirmClose(i);
        } else {
            self.close_tab(i);
        }
    }

    pub fn tab_bar(&mut self, ui: &mut Ui) {
        let th = self.th;
        let mut switch = None;
        let mut close = None;
        let mut newtab = false;
        let mut home = false;
        let hide_docs = self.placeholder_only();
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 2.0;
            ui.add_space(4.0);
            {
                let sel = self.show_home;
                let col = if sel { th.strong } else { th.dim };
                let galley = ui.painter().layout_no_wrap("Home".into(), FontId::proportional(13.0), col);
                let (rect, resp) = ui.allocate_exact_size(vec2(galley.size().x + 30.0, 28.0), Sense::click());
                let p = ui.painter();
                p.rect_filled(rect, 0.0, if sel { th.tab_sel } else if resp.hovered() { th.tab_hover } else { th.tab });
                if sel {
                    p.rect_filled(Rect::from_min_size(rect.min, vec2(rect.width(), 2.0)), 0.0, ACCENT);
                }
                p.galley(pos2(rect.min.x + 15.0, rect.center().y - galley.size().y / 2.0), galley, col);
                if resp.clicked() {
                    home = true;
                }
            }
            if !hide_docs {
                for i in 0..self.tabs.len() {
                    let (name, modified) = self.tab_title(i);
                    let sel = i == self.active && !self.show_home;
                    let label = format!("{name}{}", if modified { " *" } else { "" });
                    let col = if sel { th.strong } else { th.dim };
                    let galley = ui.painter().layout_no_wrap(label, FontId::proportional(13.0), col);
                    let size = vec2(galley.size().x + 40.0, 28.0);
                    let (rect, resp) = ui.allocate_exact_size(size, Sense::click());
                    let p = ui.painter();
                    let bg = if sel { th.tab_sel } else if resp.hovered() { th.tab_hover } else { th.tab };
                    p.rect_filled(rect, 0.0, bg);
                    if sel {
                        p.rect_filled(Rect::from_min_size(rect.min, vec2(rect.width(), 2.0)), 0.0, ACCENT);
                    }
                    p.galley(pos2(rect.min.x + 10.0, rect.center().y - galley.size().y / 2.0), galley, col);
                    let xr = Rect::from_center_size(pos2(rect.max.x - 15.0, rect.center().y), vec2(18.0, 18.0));
                    let xresp = ui.interact(xr, ui.id().with(("tabx", i)), Sense::click());
                    if xresp.hovered() {
                        ui.painter().rect_filled(xr, 3.0, th.btn_hover);
                    }
                    let c = xr.center();
                    let st = Stroke::new(1.3, th.text);
                    ui.painter().line_segment([c + vec2(-3.5, -3.5), c + vec2(3.5, 3.5)], st);
                    ui.painter().line_segment([c + vec2(-3.5, 3.5), c + vec2(3.5, -3.5)], st);
                    if xresp.clicked() || resp.middle_clicked() {
                        close = Some(i);
                    } else if resp.clicked() {
                        switch = Some(i);
                    }
                }
            }
            if ui.add_sized([28.0, 28.0], egui::Button::new("+")).on_hover_text("New project").clicked() {
                newtab = true;
            }
        });
        if home {
            self.show_home = true;
        } else if let Some(i) = close {
            self.request_close(i);
        } else if let Some(i) = switch {
            self.switch_tab(i);
            self.show_home = false;
        } else if newtab {
            self.dialog = Dialog::New { w: 64, h: 64 };
        }
    }
}

fn caption_button(ui: &mut Ui, kind: u8) -> bool {
    let (rect, resp) = ui.allocate_exact_size(vec2(46.0, 32.0), Sense::click());
    let hov = resp.hovered();
    let bg = if hov { if kind == 3 { Color32::from_rgb(196, 43, 28) } else { ui.visuals().widgets.hovered.bg_fill } } else { Color32::TRANSPARENT };
    ui.painter().rect_filled(rect, 0.0, bg);
    let c = pos2(rect.center().x.round() + 0.5, rect.center().y.round() + 0.5);
    let st = Stroke::new(1.0, if hov && kind == 3 { Color32::WHITE } else { ui.visuals().text_color() });
    let p = ui.painter();
    match kind {
        0 => {
            p.line_segment([c + vec2(-5.0, 0.0), c + vec2(5.0, 0.0)], st);
        }
        1 => {
            p.rect_stroke(Rect::from_center_size(c, vec2(10.0, 10.0)), 0.0, st);
        }
        2 => {
            p.rect_stroke(Rect::from_min_size(c + vec2(-5.0, -2.0), vec2(8.0, 8.0)), 0.0, st);
            p.line_segment([c + vec2(-2.0, -5.0), c + vec2(5.0, -5.0)], st);
            p.line_segment([c + vec2(5.0, -5.0), c + vec2(5.0, 3.0)], st);
            p.line_segment([c + vec2(-2.0, -5.0), c + vec2(-2.0, -2.0)], st);
        }
        _ => {
            p.line_segment([c + vec2(-5.0, -5.0), c + vec2(5.0, 5.0)], st);
            p.line_segment([c + vec2(-5.0, 5.0), c + vec2(5.0, -5.0)], st);
        }
    }
    resp.clicked()
}