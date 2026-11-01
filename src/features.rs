//! Feature logic added in 0.3: lasso, tags, effects, project colors, recovery, themes, drag & drop.
use super::*;
use std::collections::HashMap;

pub fn write_doc_file(doc: &Doc, path: &Path) -> Result<(), String> {
    use std::io::Write;
    let json = serde_json::to_vec(doc).map_err(|e| e.to_string())?;
    let mut enc = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
    enc.write_all(&json).map_err(|e| e.to_string())?;
    let bytes = enc.finish().map_err(|e| e.to_string())?;
    std::fs::write(path, bytes).map_err(|e| e.to_string())
}

pub fn read_doc_file(path: &Path) -> Result<Doc, String> {
    use std::io::Read;
    let bytes = std::fs::read(path).map_err(|e| e.to_string())?;
    let mut data = Vec::new();
    if flate2::read::GzDecoder::new(&bytes[..]).read_to_end(&mut data).is_err() {
        data = bytes;
    }
    let mut doc: Doc = serde_json::from_slice(&data).map_err(|e| format!("Not a SpriteCraft file: {e}"))?;
    if doc.frames.is_empty() || doc.layers.is_empty() || doc.frames.iter().any(|f| f.len() != doc.layers.len() || f.iter().any(|c| c.len() != doc.w * doc.h)) {
        return Err("Corrupt project".into());
    }
    doc.fix();
    Ok(doc)
}

impl App {
    // ------------------------------------------------------------ lasso
    pub fn update_lasso(&mut self, pts: &[(i32, i32)], base: &Option<Vec<bool>>) {
        let (w, h) = (self.doc.w, self.doc.h);
        let mut m = effects::polygon_mask(w, h, pts);
        if let Some(b) = base {
            for i in 0..w * h {
                m[i] |= b[i];
            }
        }
        self.selection = Some(m);
    }

    // ------------------------------------------------------------ new project / home
    pub fn pristine(&self) -> bool {
        !self.modified && self.path.is_none() && self.undo.is_empty()
    }

    pub fn placeholder_only(&self) -> bool {
        self.show_home && self.tabs.len() == 1 && self.pristine()
    }

    pub fn new_project(&mut self, w: usize, h: usize) {
        if self.placeholder_only() {
            self.new_doc(w, h);
        } else {
            self.new_tab(w, h);
        }
        self.show_home = false;
    }

    pub fn open_path(&mut self, p: &Path) {
        let ext = p.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
        let r = match ext.as_str() {
            "png" => self.import_png(p, true),
            _ => self.load_from(p),
        };
        if let Err(e) = r {
            self.status = e;
        }
    }

    // ------------------------------------------------------------ tags and frame names
    pub fn tag_apply(&mut self, idx: Option<usize>, name: String, from: usize, to: usize, dir: usize, color: [u8; 3]) {
        self.push_undo();
        let last = self.doc.frames.len() - 1;
        let (from, to) = (from.min(last), to.min(last));
        let t = Tag { name, from: from.min(to), to: from.max(to), dir: [TagDir::Forward, TagDir::Reverse, TagDir::PingPong][dir.min(2)], color };
        match idx {
            Some(i) if i < self.doc.tags.len() => self.doc.tags[i] = t,
            _ => self.doc.tags.push(t),
        }
        self.changed();
    }

    pub fn tag_delete(&mut self, i: usize) {
        if i < self.doc.tags.len() {
            self.push_undo();
            self.doc.tags.remove(i);
            if self.play_tag == Some(i) {
                self.play_tag = None;
            }
            self.changed();
        }
    }

    pub fn rename_frame(&mut self, name: String) {
        self.push_undo();
        self.doc.names[self.cur_frame] = name;
        self.changed();
    }

    pub fn playback_seq(&self) -> Vec<usize> {
        match self.play_tag.and_then(|i| self.doc.tags.get(i)) {
            Some(t) => self.doc.tag_sequence(t),
            None => (0..self.doc.frames.len()).collect(),
        }
    }

    pub fn tag_color(t: &Tag) -> Color32 {
        Color32::from_rgb(t.color[0], t.color[1], t.color[2])
    }

    // ------------------------------------------------------------ effects
    pub fn fx_outline(&mut self, all: bool, thick: usize, diag: bool, inner: bool) {
        let col = px_of(self.color);
        self.apply_filter(all, move |b, w, h| effects::outline(b, w, h, col, thick, diag, inner));
        self.status = "Outline applied".into();
    }

    pub fn fx_shadow(&mut self, all: bool, dx: i32, dy: i32) {
        let col = px_of(self.color2);
        self.apply_filter(all, move |b, w, h| effects::drop_shadow(b, w, h, dx, dy, col));
        self.status = "Drop shadow applied (secondary color)".into();
    }

    pub fn fx_shade(&mut self, all: bool, light: (i32, i32), depth: usize, strength: f32, hue: f32) {
        self.apply_filter(all, move |b, w, h| effects::auto_shade(b, w, h, light, depth, strength, hue));
        self.status = "Auto-shade applied".into();
    }

    pub fn rotate_layer(&mut self, angle: f32, all: bool) {
        if self.doc.layers[self.cur_layer].locked {
            self.status = "Layer is locked".into();
            return;
        }
        self.push_undo();
        let (w, h) = (self.doc.w, self.doc.h);
        let sel = self.selection.clone();
        let frames: Vec<usize> = if all { (0..self.doc.frames.len()).collect() } else { vec![self.cur_frame] };
        let mut new_sel = None;
        for fi in frames {
            let cel = Arc::make_mut(&mut self.doc.frames[fi][self.cur_layer]);
            match &sel {
                Some(m) => {
                    let Some((x0, y0, x1, y1)) = mask_bbox(m, w) else { continue };
                    let (cx, cy) = ((x0 + x1 + 1) as f32 / 2.0, (y0 + y1 + 1) as f32 / 2.0);
                    let mut tmp = vec![CLEAR; w * h];
                    for i in 0..w * h {
                        if m[i] {
                            tmp[i] = cel[i];
                            cel[i] = CLEAR;
                        }
                    }
                    let r = effects::rotsprite(&tmp, w, h, angle, cx, cy);
                    let mut nm = vec![false; w * h];
                    for i in 0..w * h {
                        if r[i][3] > 0 {
                            cel[i] = over(cel[i], r[i], 1.0);
                            nm[i] = true;
                        }
                    }
                    if fi == self.cur_frame {
                        new_sel = Some(nm);
                    }
                }
                None => {
                    let r = effects::rotsprite(cel, w, h, angle, w as f32 / 2.0, h as f32 / 2.0);
                    *cel = r;
                }
            }
        }
        if new_sel.is_some() {
            self.selection = new_sel;
        }
        self.changed_all();
        self.status = format!("Rotated {angle:.1} degrees (RotSprite)");
    }

    /// scope: 0 = this layer & frame, 1 = all layers of this frame, 2 = this layer in all frames, 3 = everything
    pub fn fx_replace(&mut self, tol: i32, scope: usize) {
        let (from, to) = (px_of(self.color), px_of(self.color2));
        self.push_undo();
        let (nf, nl) = (self.doc.frames.len(), self.doc.layers.len());
        let frames: Vec<usize> = if scope >= 2 { (0..nf).collect() } else { vec![self.cur_frame] };
        let layers: Vec<usize> = if scope == 1 || scope == 3 { (0..nl).collect() } else { vec![self.cur_layer] };
        let mut count = 0;
        for &fi in &frames {
            for &li in &layers {
                if self.doc.layers[li].locked {
                    continue;
                }
                let sel = self.selection.as_deref();
                let cel = Arc::make_mut(&mut self.doc.frames[fi][li]);
                match sel {
                    None => count += effects::replace_color(cel, from, to, tol),
                    Some(s) => {
                        for (i, p) in cel.iter_mut().enumerate() {
                            if s[i] && effects::color_close(*p, from, tol) {
                                *p = to;
                                count += 1;
                            }
                        }
                    }
                }
            }
        }
        self.changed_all();
        self.status = format!("Replaced {count} pixels");
    }

    // ------------------------------------------------------------ project colors
    pub fn refresh_project_colors(&mut self) {
        let mut m: HashMap<Px, usize> = HashMap::new();
        for fr in &self.doc.frames {
            for c in fr {
                for p in c.iter().filter(|p| p[3] > 0) {
                    *m.entry(*p).or_insert(0) += 1;
                }
            }
        }
        let mut v: Vec<(Px, usize)> = m.into_iter().collect();
        v.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
        v.truncate(512);
        self.project_colors = v;
        self.colors_dirty = false;
    }

    /// Selects every pixel equal to `c` (within `tol`) on the current layer, or on the flattened frame.
    pub fn select_color(&mut self, c: Px, tol: i32, flattened: bool) {
        let buf: Vec<Px> = if flattened { self.doc.composite(self.cur_frame) } else { self.doc.frames[self.cur_frame][self.cur_layer].to_vec() };
        let m: Vec<bool> = buf.iter().map(|p| effects::color_close(*p, c, tol)).collect();
        let n = m.iter().filter(|b| **b).count();
        self.status = format!("Selected {n} pixels");
        self.selection = if n > 0 { Some(m) } else { None };
    }

    // ------------------------------------------------------------ theme
    pub fn set_light(&mut self, ctx: &egui::Context, light: bool) {
        self.config.light = light;
        self.config.save();
        self.th = Theme::new(light);
        apply_theme(ctx, light);
    }

    // ------------------------------------------------------------ recovery
    pub fn startup(&mut self) {
        let crashed = config::find_crashed(&self.session);
        if !crashed.is_empty() {
            self.dialog = Dialog::Recover(crashed);
        }
    }

    pub fn autosave(&mut self, now: f64) {
        if now - self.last_autosave < 30.0 || self.stroke.is_some() {
            return;
        }
        self.last_autosave = now;
        let dir = config::session_dir(&self.session);
        if std::fs::create_dir_all(&dir).is_err() {
            return;
        }
        let _ = std::fs::write(dir.join("alive"), format!("{now}"));
        for i in 0..self.tabs.len() {
            let (modified, uid, path, doc) = if i == self.active {
                (self.modified, self.uid, self.path.clone(), &self.doc)
            } else {
                let t = &self.tabs[i];
                (t.modified, t.uid, t.path.clone(), &t.doc)
            };
            let f = dir.join(format!("tab-{uid}.spc"));
            if modified {
                if write_doc_file(doc, &f).is_ok() {
                    let _ = std::fs::write(f.with_extension("meta"), path.map(|p| p.to_string_lossy().to_string()).unwrap_or_default());
                }
            } else {
                let _ = std::fs::remove_file(&f);
                let _ = std::fs::remove_file(f.with_extension("meta"));
            }
        }
    }

    pub fn recover(&mut self, crashed: &[config::Crashed]) {
        let mut n = 0;
        for c in crashed {
            for (f, orig) in &c.files {
                if let Ok(doc) = read_doc_file(f) {
                    self.prepare_tab();
                    self.doc = doc;
                    self.undo.clear();
                    self.redo.clear();
                    self.cur_frame = 0;
                    self.cur_layer = 0;
                    self.selection = None;
                    self.path = orig.as_ref().map(PathBuf::from);
                    self.need_fit = true;
                    self.show_home = false;
                    self.changed_all();
                    self.modified = true;
                    n += 1;
                }
            }
            let _ = std::fs::remove_dir_all(&c.dir);
        }
        self.status = format!("Recovered {n} project(s)");
    }

    pub fn discard_recovery(&mut self, crashed: &[config::Crashed]) {
        for c in crashed {
            let _ = std::fs::remove_dir_all(&c.dir);
        }
    }

    pub fn any_modified(&self) -> bool {
        self.modified || self.tabs.iter().enumerate().any(|(i, t)| i != self.active && t.modified)
    }

    // ------------------------------------------------------------ drag & drop
    pub fn handle_drops(&mut self, ctx: &egui::Context) {
        let files = ctx.input(|i| i.raw.dropped_files.clone());
        for f in files {
            if let Some(p) = f.path {
                let ext = p.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
                if ext == "spc" || ext == "png" {
                    self.open_path(&p);
                } else {
                    self.status = format!("Can't open .{ext} files - drop .spc or .png");
                }
            }
        }
        if ctx.input(|i| !i.raw.hovered_files.is_empty()) {
            let p = ctx.layer_painter(egui::LayerId::new(egui::Order::Foreground, egui::Id::new("dropov")));
            let r = ctx.screen_rect();
            p.rect_filled(r, 0.0, Color32::from_rgba_unmultiplied(38, 120, 220, 70));
            p.rect_stroke(r.shrink(10.0), 12.0, Stroke::new(3.0, Color32::from_rgb(38, 120, 220)));
            p.text(r.center(), Align2::CENTER_CENTER, "Drop .spc or .png to open", FontId::proportional(28.0), Color32::WHITE);
        }
    }
}
