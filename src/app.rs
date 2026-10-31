use super::*;
use eframe::egui::ecolor::Hsva;
use std::cell::Cell;

#[allow(clippy::too_many_arguments)]
fn plot_ctx<'a>(w: usize, h: usize, size: i32, round: bool, dither: bool, sx: bool, sy: bool, sel: Option<&'a [bool]>, density: f32, seed: u64) -> PlotCtx<'a> {
    PlotCtx { w: w as i32, h: h as i32, size, round, dither, sx, sy, sel, density, rng: Cell::new(seed | 1) }
}

impl App {
    pub fn new(cc: &eframe::CreationContext<'_>) -> App {
        apply_theme(&cc.egui_ctx);
        let doc = Doc::new(64, 64);
        App {
            doc,
            undo: vec![],
            redo: vec![],
            cur_frame: 0,
            cur_layer: 0,
            tool: Tool::Pencil,
            color: hex_color(0xef7d57),
            color2: hex_color(0x1a1c2c),
            hex_text: String::new(),
            brush: 1,
            round: false,
            dither: false,
            pixel_perfect: true,
            sym_x: false,
            sym_y: false,
            fill_shapes: false,
            global_fill: false,
            tolerance: 0,
            shade_darken: false,
            shade_strength: 0.15,
            spray_density: 0.3,
            zoom: 8.0,
            pan: vec2(0.0, 0.0),
            need_fit: true,
            grid: true,
            rulers: true,
            onion: false,
            onion_n: 1,
            playing: false,
            play_frame: 0,
            play_last: 0.0,
            tile_preview: false,
            palette: palettes::PRESETS[0].colors.iter().map(|&c| hex_color(c)).collect(),
            palette_remove: false,
            selection: None,
            clipboard: None,
            stroke: None,
            panning: false,
            shift: false,
            hover: None,
            main_tex: None,
            prev_tex: None,
            thumbs: vec![],
            dirty: true,
            dirty_all: true,
            dialog: Dialog::None,
            status: "Welcome to SpriteCraft".into(),
            path: None,
            modified: false,
            rename_layer: None,
            rng: 0x9E3779B97F4A7C15,
            tabs: vec![TabState::empty()],
            active: 0,
            uid: 1,
            uid_counter: 1,
            logo_tex: Some(cc.egui_ctx.load_texture("logo", egui::ColorImage::from_rgba_unmultiplied([64, 64], &logo_rgba()), TextureOptions::NEAREST)),
            text: "Hello".into(),
            text_font_name: "Hack".into(),
            fonts: text::scan_fonts(),
            text_size: 16.0,
            text_smooth: false,
            text_thresh: 0.5,
            text_cache: None,
            text_font: None,
            text_tex: None,
        }
    }

    pub fn changed(&mut self) {
        self.dirty = true;
        self.modified = true;
    }

    pub fn changed_all(&mut self) {
        self.dirty = true;
        self.dirty_all = true;
        self.modified = true;
    }

    pub fn clamp_indices(&mut self) {
        self.cur_frame = self.cur_frame.min(self.doc.frames.len() - 1);
        self.cur_layer = self.cur_layer.min(self.doc.layers.len() - 1);
        self.play_frame = self.play_frame.min(self.doc.frames.len() - 1);
        if let Some(s) = &self.selection {
            if s.len() != self.doc.w * self.doc.h {
                self.selection = None;
            }
        }
    }

    pub fn push_undo(&mut self) {
        self.undo.push(self.doc.clone());
        if self.undo.len() > 120 {
            self.undo.remove(0);
        }
        self.redo.clear();
    }

    pub fn do_undo(&mut self) {
        if let Some(d) = self.undo.pop() {
            self.redo.push(std::mem::replace(&mut self.doc, d));
            self.clamp_indices();
            self.changed_all();
            self.status = "Undo".into();
        }
    }

    pub fn do_redo(&mut self) {
        if let Some(d) = self.redo.pop() {
            self.undo.push(std::mem::replace(&mut self.doc, d));
            self.clamp_indices();
            self.changed_all();
            self.status = "Redo".into();
        }
    }

    pub fn new_doc(&mut self, w: usize, h: usize) {
        if self.modified || self.path.is_some() {
            self.uid_counter += 1;
            self.uid = self.uid_counter;
        }
        self.doc = Doc::new(w, h);
        self.undo.clear();
        self.redo.clear();
        self.cur_frame = 0;
        self.cur_layer = 0;
        self.selection = None;
        self.stroke = None;
        self.path = None;
        self.modified = false;
        self.need_fit = true;
        self.playing = false;
        self.changed_all();
        self.modified = false;
        self.status = format!("New {w}x{h} sprite");
    }

    fn op_for(&self, tool: Tool, btn: u8) -> Op {
        let col = if btn == 0 { px_of(self.color) } else { px_of(self.color2) };
        match tool {
            Tool::Eraser => Op::Set(CLEAR),
            Tool::Shade => {
                let dark = self.shade_darken ^ (btn == 1);
                Op::Shade(if dark { -self.shade_strength } else { self.shade_strength })
            }
            _ => Op::Set(col),
        }
    }

    // ------------------------------------------------------------ strokes
    pub fn begin_stroke(&mut self, p: (i32, i32), btn: u8, alt: bool) {
        let drawing = matches!(self.tool, Tool::Pencil | Tool::Eraser | Tool::Bucket | Tool::Line | Tool::Rect | Tool::Ellipse | Tool::Shade | Tool::Spray);
        let tool = if alt && drawing { Tool::Picker } else { self.tool };
        let (w, h) = (self.doc.w, self.doc.h);
        let (f, l) = (self.cur_frame, self.cur_layer);
        let editing = matches!(tool, Tool::Pencil | Tool::Eraser | Tool::Bucket | Tool::Line | Tool::Rect | Tool::Ellipse | Tool::Shade | Tool::Spray | Tool::Move | Tool::Text);
        if editing && self.doc.layers[l].locked {
            self.status = "Layer is locked".into();
            return;
        }
        let inside = p.0 >= 0 && p.1 >= 0 && (p.0 as usize) < w && (p.1 as usize) < h;
        let paints = matches!(tool, Tool::Pencil | Tool::Eraser | Tool::Bucket | Tool::Line | Tool::Rect | Tool::Ellipse | Tool::Shade | Tool::Spray | Tool::Text);
        if paints && !inside {
            self.status = "Click inside the canvas to draw".into();
            return;
        }
        let col = if btn == 0 { px_of(self.color) } else { px_of(self.color2) };
        let mut st = Stroke_ { tool, button: btn, start: p, last: p, base: vec![], pts: vec![p], lifted: vec![], mask0: vec![], sel_base: None };
        let seed = self.rng;
        match tool {
            Tool::Pencil | Tool::Eraser | Tool::Shade | Tool::Spray => {
                self.push_undo();
                st.base = self.doc.frames[f][l].to_vec();
                let op = self.op_for(tool, btn);
                let dens = if tool == Tool::Spray { self.spray_density } else { 1.0 };
                let ctx = plot_ctx(w, h, self.brush, self.round, self.dither, self.sym_x, self.sym_y, self.selection.as_deref(), dens, seed);
                let cel = Arc::make_mut(&mut self.doc.frames[f][l]);
                ctx.put_brush(cel, p.0, p.1, op);
                self.rng = ctx.rng.get();
                self.changed();
            }
            Tool::Bucket => {
                self.push_undo();
                let mask = flood_mask(&self.doc.frames[f][l], w, h, p.0, p.1, self.tolerance, self.global_fill);
                let sel = self.selection.as_deref();
                let cel = Arc::make_mut(&mut self.doc.frames[f][l]);
                for i in 0..w * h {
                    if mask[i] && sel.map_or(true, |s| s[i]) {
                        cel[i] = col;
                    }
                }
                self.changed();
                return;
            }
            Tool::Picker => {
                if p.0 >= 0 && p.1 >= 0 && (p.0 as usize) < w && (p.1 as usize) < h {
                    let comp = self.doc.composite(f);
                    let c = comp[p.1 as usize * w + p.0 as usize];
                    if c[3] > 0 {
                        if btn == 0 { self.color = color_of(c) } else { self.color2 = color_of(c) }
                    } else {
                        self.status = "Transparent pixel".into();
                    }
                }
                return;
            }
            Tool::Line | Tool::Rect | Tool::Ellipse => {
                self.push_undo();
                st.base = self.doc.frames[f][l].to_vec();
                self.redraw_shape(&st, p);
            }
            Tool::Select => {
                st.sel_base = if self.shift { self.selection.clone() } else { None };
                self.update_select(&st, p);
            }
            Tool::Wand => {
                let m = flood_mask(&self.doc.frames[f][l], w, h, p.0, p.1, self.tolerance, self.global_fill);
                let prev = self.selection.clone();
                let out: Vec<bool> = match (&prev, self.shift, alt) {
                    (Some(pv), true, _) => (0..w * h).map(|i| pv[i] || m[i]).collect(),
                    (Some(pv), false, true) => (0..w * h).map(|i| pv[i] && !m[i]).collect(),
                    _ => m,
                };
                self.selection = if out.iter().any(|&b| b) { Some(out) } else { None };
                return;
            }
            Tool::Move => {
                self.push_undo();
                let mask = self.selection.clone().unwrap_or_else(|| vec![true; w * h]);
                let mut base = self.doc.frames[f][l].to_vec();
                for i in 0..w * h {
                    if mask[i] {
                        if base[i][3] > 0 {
                            st.lifted.push(((i % w) as i32, (i / w) as i32, base[i]));
                        }
                        base[i] = CLEAR;
                    }
                }
                st.base = base;
                st.mask0 = mask;
            }
            Tool::Text => {
                self.stamp_text(p);
                return;
            }
            Tool::Hand => return,
        }
        self.stroke = Some(st);
    }

    fn redraw_shape(&mut self, st: &Stroke_, p: (i32, i32)) {
        let (w, h) = (self.doc.w, self.doc.h);
        let col = if st.button == 0 { px_of(self.color) } else { px_of(self.color2) };
        let shape = match st.tool {
            Tool::Line => Shape::Line,
            Tool::Rect => Shape::Rect,
            _ => Shape::Ellipse,
        };
        let p = (p.0.clamp(0, w as i32 - 1), p.1.clamp(0, h as i32 - 1));
        let ctx = plot_ctx(w, h, self.brush, self.round, self.dither, self.sym_x, self.sym_y, self.selection.as_deref(), 1.0, self.rng);
        let cel = Arc::make_mut(&mut self.doc.frames[self.cur_frame][self.cur_layer]);
        cel.copy_from_slice(&st.base);
        draw_shape(cel, &ctx, shape, st.start, p, self.fill_shapes, col, self.shift);
        self.changed();
    }

    fn update_select(&mut self, st: &Stroke_, p: (i32, i32)) {
        let (w, h) = (self.doc.w, self.doc.h);
        let mut m = rect_mask(w, h, st.start, p);
        if let Some(b) = &st.sel_base {
            for i in 0..w * h {
                m[i] |= b[i];
            }
        }
        self.selection = Some(m);
    }

    pub fn drag_stroke(&mut self, p: (i32, i32)) {
        let Some(mut st) = self.stroke.take() else { return };
        let (w, h) = (self.doc.w, self.doc.h);
        let (f, l) = (self.cur_frame, self.cur_layer);
        let same = p == st.last;
        match st.tool {
            Tool::Pencil | Tool::Eraser | Tool::Shade | Tool::Spray => {
                if !(same && st.tool != Tool::Spray) {
                    let op = self.op_for(st.tool, st.button);
                    let dens = if st.tool == Tool::Spray { self.spray_density } else { 1.0 };
                    let pp = st.tool == Tool::Pencil && self.pixel_perfect && self.brush == 1 && !self.sym_x && !self.sym_y && !self.dither;
                    let ctx = plot_ctx(w, h, self.brush, self.round, self.dither, self.sym_x, self.sym_y, self.selection.as_deref(), dens, self.rng);
                    let cel = Arc::make_mut(&mut self.doc.frames[f][l]);
                    let pts = if same { vec![p] } else { line_points(st.last, p).into_iter().skip(1).collect() };
                    for q in pts {
                        ctx.put_brush(cel, q.0, q.1, op);
                        if pp {
                            st.pts.push(q);
                            let n = st.pts.len();
                            if n >= 3 {
                                let (a, b, c) = (st.pts[n - 3], st.pts[n - 2], st.pts[n - 1]);
                                let ab = (b.0 - a.0, b.1 - a.1);
                                let bc = (c.0 - b.0, c.1 - b.1);
                                if ab.0.abs() + ab.1.abs() == 1 && bc.0.abs() + bc.1.abs() == 1 && ab.0 * bc.0 + ab.1 * bc.1 == 0 {
                                    if b.0 >= 0 && b.1 >= 0 && (b.0 as usize) < w && (b.1 as usize) < h {
                                        let i = b.1 as usize * w + b.0 as usize;
                                        cel[i] = st.base[i];
                                    }
                                    st.pts.remove(n - 2);
                                }
                            }
                        }
                    }
                    self.rng = ctx.rng.get();
                    self.changed();
                }
            }
            Tool::Line | Tool::Rect | Tool::Ellipse => {
                self.redraw_shape(&st, p);
            }
            Tool::Select => self.update_select(&st, p),
            Tool::Move => {
                let (dx, dy) = (p.0 - st.start.0, p.1 - st.start.1);
                let cel = Arc::make_mut(&mut self.doc.frames[f][l]);
                cel.copy_from_slice(&st.base);
                for &(x, y, c) in &st.lifted {
                    let (nx, ny) = (x + dx, y + dy);
                    if nx >= 0 && ny >= 0 && (nx as usize) < w && (ny as usize) < h {
                        let i = ny as usize * w + nx as usize;
                        cel[i] = over(cel[i], c, 1.0);
                    }
                }
                if self.selection.is_some() {
                    let mut m = vec![false; w * h];
                    for i in 0..w * h {
                        if st.mask0[i] {
                            let (nx, ny) = ((i % w) as i32 + dx, (i / w) as i32 + dy);
                            if nx >= 0 && ny >= 0 && (nx as usize) < w && (ny as usize) < h {
                                m[ny as usize * w + nx as usize] = true;
                            }
                        }
                    }
                    self.selection = Some(m);
                }
                self.changed();
            }
            _ => {}
        }
        st.last = p;
        self.stroke = Some(st);
    }

    pub fn end_stroke(&mut self) {
        if let Some(st) = self.stroke.take() {
            if st.tool == Tool::Select && st.start == st.last && !self.shift {
                self.selection = None;
            }
            self.changed();
        }
    }

    // ------------------------------------------------------------ selection / clipboard
    pub fn select_all(&mut self) {
        self.selection = Some(vec![true; self.doc.w * self.doc.h]);
    }

    pub fn invert_selection(&mut self) {
        if let Some(s) = &mut self.selection {
            for b in s.iter_mut() {
                *b = !*b;
            }
        }
    }

    pub fn select_opaque(&mut self) {
        let m: Vec<bool> = self.doc.frames[self.cur_frame][self.cur_layer].iter().map(|p| p[3] > 0).collect();
        self.selection = Some(m);
    }

    pub fn copy_sel(&mut self, cut: bool) {
        let (w, h) = (self.doc.w, self.doc.h);
        let mask = self.selection.clone().unwrap_or_else(|| vec![true; w * h]);
        let Some((x0, y0, x1, y1)) = mask_bbox(&mask, w) else { return };
        let (cw, ch) = (x1 - x0 + 1, y1 - y0 + 1);
        let mut px = vec![CLEAR; cw * ch];
        let src = self.doc.frames[self.cur_frame][self.cur_layer].clone();
        for y in 0..ch {
            for x in 0..cw {
                let i = (y + y0) * w + x + x0;
                if mask[i] {
                    px[y * cw + x] = src[i];
                }
            }
        }
        self.clipboard = Some(Clip { w: cw, h: ch, px });
        if cut {
            self.clear_sel();
        }
        self.status = if cut { "Cut".into() } else { "Copied".into() };
    }

    pub fn clear_sel(&mut self) {
        if self.doc.layers[self.cur_layer].locked {
            return;
        }
        self.push_undo();
        let sel = self.selection.as_deref();
        let cel = Arc::make_mut(&mut self.doc.frames[self.cur_frame][self.cur_layer]);
        for (i, p) in cel.iter_mut().enumerate() {
            if sel.map_or(true, |s| s[i]) {
                *p = CLEAR;
            }
        }
        self.changed_all();
    }

    pub fn paste(&mut self) {
        let Some(c) = self.clipboard.as_ref() else {
            self.status = "Clipboard is empty".into();
            return;
        };
        if self.doc.layers[self.cur_layer].locked {
            return;
        }
        let (cw, ch, px) = (c.w, c.h, c.px.clone());
        self.push_undo();
        let (w, h) = (self.doc.w, self.doc.h);
        let mut mask = vec![false; w * h];
        let cel = Arc::make_mut(&mut self.doc.frames[self.cur_frame][self.cur_layer]);
        for y in 0..ch.min(h) {
            for x in 0..cw.min(w) {
                let p = px[y * cw + x];
                if p[3] > 0 {
                    cel[y * w + x] = over(cel[y * w + x], p, 1.0);
                    mask[y * w + x] = true;
                }
            }
        }
        self.selection = Some(mask);
        self.tool = Tool::Move;
        self.changed_all();
        self.status = "Pasted - drag with the Move tool".into();
    }

    pub fn crop_to_selection(&mut self) {
        let Some(sel) = &self.selection else { return };
        let Some((x0, y0, x1, y1)) = mask_bbox(sel, self.doc.w) else { return };
        self.push_undo();
        self.doc.resize_canvas(x1 - x0 + 1, y1 - y0 + 1, -(x0 as i32), -(y0 as i32));
        self.selection = None;
        self.need_fit = true;
        self.changed_all();
    }

    // ------------------------------------------------------------ layers & frames
    pub fn layer_add(&mut self) {
        self.push_undo();
        self.cur_layer = self.doc.add_layer(self.cur_layer);
        self.changed_all();
    }
    pub fn layer_dup(&mut self) {
        self.push_undo();
        self.cur_layer = self.doc.duplicate_layer(self.cur_layer);
        self.changed_all();
    }
    pub fn layer_del(&mut self) {
        if self.doc.layers.len() > 1 {
            self.push_undo();
            self.doc.delete_layer(self.cur_layer);
            self.clamp_indices();
            self.changed_all();
        }
    }
    pub fn layer_move(&mut self, up: bool) {
        self.push_undo();
        self.cur_layer = self.doc.move_layer(self.cur_layer, up);
        self.changed_all();
    }
    pub fn layer_merge(&mut self) {
        if self.cur_layer > 0 {
            self.push_undo();
            self.cur_layer = self.doc.merge_down(self.cur_layer);
            self.changed_all();
        }
    }
    pub fn flatten(&mut self) {
        self.push_undo();
        self.doc.flatten();
        self.cur_layer = 0;
        self.changed_all();
    }
    pub fn frame_add(&mut self) {
        self.push_undo();
        self.cur_frame = self.doc.add_frame(self.cur_frame);
        self.changed_all();
    }
    pub fn frame_dup(&mut self) {
        self.push_undo();
        self.cur_frame = self.doc.duplicate_frame(self.cur_frame);
        self.changed_all();
    }
    pub fn frame_del(&mut self) {
        if self.doc.frames.len() > 1 {
            self.push_undo();
            self.doc.delete_frame(self.cur_frame);
            self.clamp_indices();
            self.changed_all();
        }
    }
    pub fn frame_move(&mut self, right: bool) {
        self.push_undo();
        self.cur_frame = self.doc.move_frame(self.cur_frame, right);
        self.changed_all();
    }
    pub fn frame_reverse(&mut self) {
        self.push_undo();
        self.doc.reverse_frames();
        self.cur_frame = self.doc.frames.len() - 1 - self.cur_frame;
        self.changed_all();
    }
    pub fn frame_pingpong(&mut self) {
        self.push_undo();
        self.doc.pingpong();
        self.changed_all();
    }
    pub fn goto_frame(&mut self, delta: i32) {
        let n = self.doc.frames.len() as i32;
        self.cur_frame = ((self.cur_frame as i32 + delta).rem_euclid(n)) as usize;
        self.dirty = true;
    }

    // ------------------------------------------------------------ transforms
    pub fn flip_canvas(&mut self, horizontal: bool) {
        self.push_undo();
        self.doc.flip_canvas(horizontal);
        self.selection = None;
        self.changed_all();
    }
    pub fn rotate_canvas(&mut self, cw: bool) {
        self.push_undo();
        self.doc.rotate_canvas(cw);
        self.selection = None;
        self.need_fit = true;
        self.changed_all();
    }
    pub fn flip_layer(&mut self, horizontal: bool) {
        if self.doc.layers[self.cur_layer].locked {
            return;
        }
        self.apply_filter(false, move |b, w, h| {
            let n = flip_buf(b, w, h, horizontal);
            b.copy_from_slice(&n);
        });
    }

    // ------------------------------------------------------------ filters
    pub fn apply_filter<F: Fn(&mut Vec<Px>, usize, usize)>(&mut self, all: bool, f: F) {
        if self.doc.layers[self.cur_layer].locked {
            self.status = "Layer is locked".into();
            return;
        }
        self.push_undo();
        let (w, h) = (self.doc.w, self.doc.h);
        let frames: Vec<usize> = if all { (0..self.doc.frames.len()).collect() } else { vec![self.cur_frame] };
        for fi in frames {
            let cel = Arc::make_mut(&mut self.doc.frames[fi][self.cur_layer]);
            let orig = cel.clone();
            f(cel, w, h);
            if let Some(sel) = &self.selection {
                for i in 0..w * h {
                    if !sel[i] {
                        cel[i] = orig[i];
                    }
                }
            }
        }
        self.changed_all();
    }

    pub fn filter_invert(&mut self, all: bool) {
        self.apply_filter(all, |b, _, _| {
            for p in b.iter_mut().filter(|p| p[3] > 0) {
                p[0] = 255 - p[0];
                p[1] = 255 - p[1];
                p[2] = 255 - p[2];
            }
        });
    }

    pub fn filter_gray(&mut self, all: bool) {
        self.apply_filter(all, |b, _, _| {
            for p in b.iter_mut().filter(|p| p[3] > 0) {
                let g = (0.299 * p[0] as f32 + 0.587 * p[1] as f32 + 0.114 * p[2] as f32).round() as u8;
                p[0] = g;
                p[1] = g;
                p[2] = g;
            }
        });
    }

    pub fn filter_hsv(&mut self, all: bool, hue: f32, sat: f32, val: f32) {
        self.apply_filter(all, move |b, _, _| {
            for p in b.iter_mut().filter(|p| p[3] > 0) {
                let mut c = Hsva::from_srgba_unmultiplied(*p);
                c.h = (c.h + hue / 360.0).rem_euclid(1.0);
                c.s = (c.s * (1.0 + sat / 100.0)).clamp(0.0, 1.0);
                c.v = (c.v + val / 100.0).clamp(0.0, 1.0);
                let a = p[3];
                *p = c.to_srgba_unmultiplied();
                p[3] = a;
            }
        });
    }

    pub fn filter_outline(&mut self, all: bool) {
        let col = px_of(self.color);
        self.apply_filter(all, move |b, w, h| {
            let src = b.clone();
            for y in 0..h {
                for x in 0..w {
                    if src[y * w + x][3] != 0 {
                        continue;
                    }
                    let n = [(x as i32 - 1, y as i32), (x as i32 + 1, y as i32), (x as i32, y as i32 - 1), (x as i32, y as i32 + 1)];
                    if n.iter().any(|&(nx, ny)| nx >= 0 && ny >= 0 && (nx as usize) < w && (ny as usize) < h && src[ny as usize * w + nx as usize][3] != 0) {
                        b[y * w + x] = col;
                    }
                }
            }
        });
    }

    pub fn filter_replace(&mut self, all: bool) {
        let (from, to) = (px_of(self.color), px_of(self.color2));
        self.apply_filter(all, move |b, _, _| {
            for p in b.iter_mut().filter(|p| **p == from) {
                *p = to;
            }
        });
    }

    pub fn filter_quantize(&mut self, all: bool) {
        let pal: Vec<Px> = self.palette.iter().map(|&c| px_of(c)).collect();
        if pal.is_empty() {
            return;
        }
        self.apply_filter(all, move |b, _, _| {
            for p in b.iter_mut().filter(|p| p[3] > 0) {
                let best = pal
                    .iter()
                    .min_by_key(|q| (0..3).map(|i| (q[i] as i32 - p[i] as i32).pow(2)).sum::<i32>())
                    .unwrap();
                p[0] = best[0];
                p[1] = best[1];
                p[2] = best[2];
            }
        });
    }

    pub fn extract_palette(&mut self) {
        let comp = self.doc.composite(self.cur_frame);
        let mut pal: Vec<Color32> = vec![];
        for p in comp.iter().filter(|p| p[3] == 255) {
            let c = color_of(*p);
            if !pal.contains(&c) {
                pal.push(c);
                if pal.len() >= 256 {
                    break;
                }
            }
        }
        if !pal.is_empty() {
            self.status = format!("Extracted {} colors", pal.len());
            self.palette = pal;
        }
    }

    // ------------------------------------------------------------ file IO
    pub fn save_to(&mut self, path: &Path) -> Result<(), String> {
        use std::io::Write;
        let json = serde_json::to_vec(&self.doc).map_err(|e| e.to_string())?;
        let mut enc = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
        enc.write_all(&json).map_err(|e| e.to_string())?;
        let bytes = enc.finish().map_err(|e| e.to_string())?;
        std::fs::write(path, bytes).map_err(|e| e.to_string())?;
        self.path = Some(path.to_path_buf());
        self.modified = false;
        self.status = format!("Saved {}", path.display());
        Ok(())
    }

    pub fn load_from(&mut self, path: &Path) -> Result<(), String> {
        use std::io::Read;
        let bytes = std::fs::read(path).map_err(|e| e.to_string())?;
        let mut data = Vec::new();
        if flate2::read::GzDecoder::new(&bytes[..]).read_to_end(&mut data).is_err() {
            data = bytes;
        }
        let doc: Doc = serde_json::from_slice(&data).map_err(|e| format!("Not a SpriteCraft file: {e}"))?;
        if doc.frames.is_empty() || doc.layers.is_empty() || doc.frames.iter().any(|f| f.len() != doc.layers.len() || f.iter().any(|c| c.len() != doc.w * doc.h)) {
            return Err("Corrupt project".into());
        }
        self.prepare_tab();
        self.doc = doc;
        self.undo.clear();
        self.redo.clear();
        self.cur_frame = 0;
        self.cur_layer = 0;
        self.selection = None;
        self.path = Some(path.to_path_buf());
        self.need_fit = true;
        self.changed_all();
        self.modified = false;
        self.status = format!("Opened {}", path.display());
        Ok(())
    }

    pub fn import_png(&mut self, path: &Path, resize: bool) -> Result<(), String> {
        let img = image::open(path).map_err(|e| e.to_string())?.to_rgba8();
        let (iw, ih) = (img.width() as usize, img.height() as usize);
        if resize {
            self.prepare_tab();
            self.new_doc(iw, ih);
        } else {
            self.push_undo();
        }
        let (w, h) = (self.doc.w, self.doc.h);
        let cel = Arc::make_mut(&mut self.doc.frames[self.cur_frame][self.cur_layer]);
        for y in 0..ih.min(h) {
            for x in 0..iw.min(w) {
                cel[y * w + x] = img.get_pixel(x as u32, y as u32).0;
            }
        }
        self.changed_all();
        self.status = format!("Imported {}", path.display());
        Ok(())
    }

    pub fn export(&mut self, st: &ExportState) -> Result<String, String> {
        let dir = PathBuf::from(&st.dir);
        std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        let (w, h) = (self.doc.w, self.doc.h);
        let s = st.scale.max(1) as usize;
        let n = self.doc.frames.len();
        let stem = st.name.trim_end_matches(".png").trim_end_matches(".gif").to_string();
        match st.kind {
            0 => {
                let comp = self.doc.composite(self.cur_frame);
                let p = path_join(&st.dir, &sanitize_ext(&st.name, "png"));
                scaled_image(&comp, w, h, s).save(&p).map_err(|e| e.to_string())?;
                Ok(p.display().to_string())
            }
            1 => {
                let cols = st.cols.clamp(1, n);
                let rows = (n + cols - 1) / cols;
                let (fw, fh) = (w * s, h * s);
                let mut sheet = image::RgbaImage::new((cols * fw) as u32, (rows * fh) as u32);
                let mut metas = vec![];
                for i in 0..n {
                    let img = scaled_image(&self.doc.composite(i), w, h, s);
                    let (ox, oy) = ((i % cols) * fw, (i / cols) * fh);
                    image::imageops::replace(&mut sheet, &img, ox as i64, oy as i64);
                    metas.push(serde_json::json!({"frame": i, "x": ox, "y": oy, "w": fw, "h": fh, "duration_ms": self.doc.holds[i] * 1000 / self.doc.fps.max(1)}));
                }
                let p = path_join(&st.dir, &sanitize_ext(&stem, "png"));
                sheet.save(&p).map_err(|e| e.to_string())?;
                if st.json {
                    let meta = serde_json::json!({"image": p.file_name().map(|f| f.to_string_lossy().to_string()), "frame_width": fw, "frame_height": fh, "columns": cols, "fps": self.doc.fps, "frames": metas});
                    let jp = path_join(&st.dir, &format!("{stem}.json"));
                    std::fs::write(&jp, serde_json::to_string_pretty(&meta).unwrap()).map_err(|e| e.to_string())?;
                }
                Ok(p.display().to_string())
            }
            2 => {
                use image::codecs::gif::{GifEncoder, Repeat};
                let p = path_join(&st.dir, &sanitize_ext(&stem, "gif"));
                let file = std::fs::File::create(&p).map_err(|e| e.to_string())?;
                let mut enc = GifEncoder::new_with_speed(file, 10);
                enc.set_repeat(Repeat::Infinite).map_err(|e| e.to_string())?;
                for i in 0..n {
                    let img = scaled_image(&self.doc.composite(i), w, h, s);
                    let ms = self.doc.holds[i] * 1000 / self.doc.fps.max(1);
                    let fr = image::Frame::from_parts(img, 0, 0, image::Delay::from_numer_denom_ms(ms, 1));
                    enc.encode_frame(fr).map_err(|e| e.to_string())?;
                }
                Ok(p.display().to_string())
            }
            _ => {
                for i in 0..n {
                    let img = scaled_image(&self.doc.composite(i), w, h, s);
                    img.save(path_join(&st.dir, &format!("{stem}_{:03}.png", i + 1))).map_err(|e| e.to_string())?;
                }
                Ok(format!("{} ({} files)", st.dir, n))
            }
        }
    }

    // ------------------------------------------------------------ tabs
    fn tab_take(&mut self) -> TabState {
        TabState {
            doc: std::mem::replace(&mut self.doc, Doc::new(1, 1)),
            undo: std::mem::take(&mut self.undo),
            redo: std::mem::take(&mut self.redo),
            cur_frame: self.cur_frame,
            cur_layer: self.cur_layer,
            selection: self.selection.take(),
            path: self.path.take(),
            modified: self.modified,
            zoom: self.zoom,
            pan: self.pan,
            need_fit: self.need_fit,
            uid: self.uid,
        }
    }

    fn tab_put(&mut self, t: TabState) {
        self.doc = t.doc;
        self.undo = t.undo;
        self.redo = t.redo;
        self.cur_frame = t.cur_frame;
        self.cur_layer = t.cur_layer;
        self.selection = t.selection;
        self.path = t.path;
        self.modified = t.modified;
        self.zoom = t.zoom;
        self.pan = t.pan;
        self.need_fit = t.need_fit;
        self.uid = t.uid;
        self.stroke = None;
        self.playing = false;
        self.rename_layer = None;
        self.hover = None;
        self.thumbs.clear();
        self.dirty = true;
        self.dirty_all = true;
        self.clamp_indices();
    }

    pub fn switch_tab(&mut self, idx: usize) {
        if idx == self.active || idx >= self.tabs.len() {
            return;
        }
        let cur = self.tab_take();
        self.tabs[self.active] = cur;
        let t = std::mem::replace(&mut self.tabs[idx], TabState::empty());
        self.tab_put(t);
        self.active = idx;
    }

    pub fn new_tab(&mut self, w: usize, h: usize) {
        let cur = self.tab_take();
        self.tabs[self.active] = cur;
        self.tabs.push(TabState::empty());
        self.active = self.tabs.len() - 1;
        self.uid_counter += 1;
        self.uid = self.uid_counter;
        self.modified = false;
        self.path = None;
        self.new_doc(w, h);
        self.thumbs.clear();
    }

    /// Opening a file reuses the current tab only if it is still a pristine, untouched document.
    pub fn prepare_tab(&mut self) {
        let pristine = !self.modified && self.path.is_none() && self.undo.is_empty();
        if !pristine {
            self.new_tab(1, 1);
        }
    }

    pub fn close_tab(&mut self, idx: usize) {
        if self.tabs.len() == 1 {
            self.modified = false;
            self.path = None;
            self.new_doc(64, 64);
            return;
        }
        self.tabs.remove(idx);
        if idx == self.active {
            let ni = idx.min(self.tabs.len() - 1);
            let t = std::mem::replace(&mut self.tabs[ni], TabState::empty());
            self.tab_put(t);
            self.active = ni;
        } else if idx < self.active {
            self.active -= 1;
        }
    }

    pub fn tab_title(&self, i: usize) -> (String, bool) {
        let (path, uid, modified) = if i == self.active {
            (&self.path, self.uid, self.modified)
        } else {
            let t = &self.tabs[i];
            (&t.path, t.uid, t.modified)
        };
        let name = path.as_ref().and_then(|p| p.file_name()).map(|s| s.to_string_lossy().to_string()).unwrap_or_else(|| format!("Untitled-{uid}"));
        (name, modified)
    }

    // ------------------------------------------------------------ frames (extra)
    pub fn frame_clear(&mut self) {
        self.push_undo();
        let blank = self.doc.blank();
        for c in self.doc.frames[self.cur_frame].iter_mut() {
            *c = blank.clone();
        }
        self.changed_all();
    }

    pub fn frame_insert_before(&mut self) {
        self.push_undo();
        let f = self.cur_frame;
        let n = self.doc.add_frame(f);
        self.cur_frame = self.doc.move_frame(n, false);
        self.changed_all();
    }

    pub fn hold_adjust(&mut self, d: i32) {
        let h = &mut self.doc.holds[self.cur_frame];
        *h = (*h as i32 + d).clamp(1, 32) as u32;
    }

    // ------------------------------------------------------------ text
    pub fn ensure_text(&mut self) {
        let col = px_of(self.color);
        let key = format!("{}|{}|{}|{}|{}|{:?}", self.text, self.text_font_name, self.text_size, self.text_smooth, self.text_thresh, col);
        if self.text_cache.as_ref().map_or(false, |c| c.key == key) {
            return;
        }
        if self.text_font.as_ref().map_or(true, |f| f.0 != self.text_font_name) {
            self.text_font = text::load_font(&self.text_font_name, &self.fonts).map(|f| (self.text_font_name.clone(), f));
        }
        self.text_cache = match &self.text_font {
            Some((_, f)) => text::render_text(f, &self.text, self.text_size, self.text_smooth, self.text_thresh, col).map(|(w, h, px)| TextCache { key, w, h, px }),
            None => None,
        };
    }

    pub fn stamp_text(&mut self, p: (i32, i32)) {
        self.ensure_text();
        let (cw, ch, px) = match &self.text_cache {
            Some(c) => (c.w, c.h, c.px.clone()),
            None => {
                self.status = "Type some text in the options bar first".into();
                return;
            }
        };
        self.push_undo();
        let (w, h) = (self.doc.w as i32, self.doc.h as i32);
        let sel = self.selection.as_deref();
        let cel = Arc::make_mut(&mut self.doc.frames[self.cur_frame][self.cur_layer]);
        for y in 0..ch as i32 {
            for x in 0..cw as i32 {
                let q = px[y as usize * cw + x as usize];
                let (tx, ty) = (p.0 + x, p.1 + y);
                if q[3] == 0 || tx < 0 || ty < 0 || tx >= w || ty >= h {
                    continue;
                }
                let i = (ty * w + tx) as usize;
                if sel.map_or(true, |s| s[i]) {
                    cel[i] = over(cel[i], q, 1.0);
                }
            }
        }
        self.changed();
        self.status = "Text placed (Ctrl+Z to undo)".into();
    }
}
