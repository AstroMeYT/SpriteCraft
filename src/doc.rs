//! Document model (layers x frames), compositing and pixel algorithms.
use serde::{Deserialize, Serialize};
use std::sync::Arc;

pub type Px = [u8; 4];
pub const CLEAR: Px = [0, 0, 0, 0];
/// Copy-on-write pixel buffer: undo snapshots share unchanged cels.
pub type Cel = Arc<Vec<Px>>;

#[derive(Clone, Serialize, Deserialize)]
pub struct Layer {
    pub name: String,
    pub visible: bool,
    pub locked: bool,
    pub opacity: f32,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Doc {
    pub w: usize,
    pub h: usize,
    pub layers: Vec<Layer>,
    /// frames[frame][layer]
    pub frames: Vec<Vec<Cel>>,
    pub holds: Vec<u32>,
    pub fps: u32,
}

pub fn over(dst: Px, src: Px, op: f32) -> Px {
    let sa = src[3] as f32 / 255.0 * op;
    if sa <= 0.0 {
        return dst;
    }
    let da = dst[3] as f32 / 255.0;
    let oa = sa + da * (1.0 - sa);
    if oa <= 0.0 {
        return CLEAR;
    }
    let mut o = [0u8; 4];
    for i in 0..3 {
        let c = (src[i] as f32 * sa + dst[i] as f32 * da * (1.0 - sa)) / oa;
        o[i] = c.round().clamp(0.0, 255.0) as u8;
    }
    o[3] = (oa * 255.0).round().clamp(0.0, 255.0) as u8;
    o
}

impl Doc {
    pub fn new(w: usize, h: usize) -> Doc {
        Doc {
            w,
            h,
            layers: vec![Layer { name: "Layer 1".into(), visible: true, locked: false, opacity: 1.0 }],
            frames: vec![vec![Arc::new(vec![CLEAR; w * h])]],
            holds: vec![1],
            fps: 8,
        }
    }

    pub fn blank(&self) -> Cel {
        Arc::new(vec![CLEAR; self.w * self.h])
    }

    pub fn composite(&self, f: usize) -> Vec<Px> {
        let mut out = vec![CLEAR; self.w * self.h];
        for (l, layer) in self.layers.iter().enumerate() {
            if !layer.visible {
                continue;
            }
            let cel = &self.frames[f][l];
            for (o, s) in out.iter_mut().zip(cel.iter()) {
                if s[3] != 0 {
                    *o = over(*o, *s, layer.opacity);
                }
            }
        }
        out
    }

    // ---------- layers ----------
    pub fn add_layer(&mut self, above: usize) -> usize {
        let at = above + 1;
        let name = format!("Layer {}", self.layers.len() + 1);
        self.layers.insert(at, Layer { name, visible: true, locked: false, opacity: 1.0 });
        let blank = self.blank();
        for fr in &mut self.frames {
            fr.insert(at, blank.clone());
        }
        at
    }

    pub fn duplicate_layer(&mut self, l: usize) -> usize {
        let mut layer = self.layers[l].clone();
        layer.name = format!("{} copy", layer.name);
        self.layers.insert(l + 1, layer);
        for fr in &mut self.frames {
            let c = fr[l].clone();
            fr.insert(l + 1, c);
        }
        l + 1
    }

    pub fn delete_layer(&mut self, l: usize) {
        if self.layers.len() <= 1 {
            return;
        }
        self.layers.remove(l);
        for fr in &mut self.frames {
            fr.remove(l);
        }
    }

    pub fn move_layer(&mut self, l: usize, up: bool) -> usize {
        let t = if up { l + 1 } else { l.wrapping_sub(1) };
        if t >= self.layers.len() {
            return l;
        }
        self.layers.swap(l, t);
        for fr in &mut self.frames {
            fr.swap(l, t);
        }
        t
    }

    pub fn merge_down(&mut self, l: usize) -> usize {
        if l == 0 {
            return 0;
        }
        let op = self.layers[l].opacity;
        let vis = self.layers[l].visible;
        if vis {
            for fr in &mut self.frames {
                let top = fr[l].clone();
                let below = Arc::make_mut(&mut fr[l - 1]);
                for (b, t) in below.iter_mut().zip(top.iter()) {
                    if t[3] != 0 {
                        *b = over(*b, *t, op);
                    }
                }
            }
        }
        self.delete_layer(l);
        l - 1
    }

    pub fn flatten(&mut self) {
        for f in 0..self.frames.len() {
            let c = self.composite(f);
            self.frames[f] = vec![Arc::new(c)];
        }
        self.layers = vec![Layer { name: "Flattened".into(), visible: true, locked: false, opacity: 1.0 }];
    }

    // ---------- frames ----------
    pub fn add_frame(&mut self, after: usize) -> usize {
        let fr: Vec<Cel> = (0..self.layers.len()).map(|_| self.blank()).collect();
        self.frames.insert(after + 1, fr);
        self.holds.insert(after + 1, 1);
        after + 1
    }

    pub fn duplicate_frame(&mut self, f: usize) -> usize {
        let fr = self.frames[f].clone();
        let hold = self.holds[f];
        self.frames.insert(f + 1, fr);
        self.holds.insert(f + 1, hold);
        f + 1
    }

    pub fn delete_frame(&mut self, f: usize) {
        if self.frames.len() <= 1 {
            return;
        }
        self.frames.remove(f);
        self.holds.remove(f);
    }

    pub fn move_frame(&mut self, f: usize, right: bool) -> usize {
        let t = if right { f + 1 } else { f.wrapping_sub(1) };
        if t >= self.frames.len() {
            return f;
        }
        self.frames.swap(f, t);
        self.holds.swap(f, t);
        t
    }

    pub fn reverse_frames(&mut self) {
        self.frames.reverse();
        self.holds.reverse();
    }

    pub fn pingpong(&mut self) {
        let n = self.frames.len();
        if n < 3 {
            return;
        }
        for i in (1..n - 1).rev() {
            let fr = self.frames[i].clone();
            let h = self.holds[i];
            self.frames.push(fr);
            self.holds.push(h);
        }
    }

    // ---------- canvas transforms ----------
    fn map_all<F: Fn(&[Px]) -> Vec<Px>>(&mut self, f: F) {
        for fr in &mut self.frames {
            for cel in fr.iter_mut() {
                *cel = Arc::new(f(cel));
            }
        }
    }

    pub fn flip_canvas(&mut self, horizontal: bool) {
        let (w, h) = (self.w, self.h);
        self.map_all(|c| flip_buf(c, w, h, horizontal));
    }

    pub fn rotate_canvas(&mut self, cw: bool) {
        let (w, h) = (self.w, self.h);
        self.map_all(|c| {
            let mut n = vec![CLEAR; w * h];
            for y in 0..h {
                for x in 0..w {
                    let ni = if cw { x * h + (h - 1 - y) } else { (w - 1 - x) * h + y };
                    n[ni] = c[y * w + x];
                }
            }
            n
        });
        self.w = h;
        self.h = w;
    }

    pub fn resize_canvas(&mut self, nw: usize, nh: usize, ox: i32, oy: i32) {
        let (w, h) = (self.w, self.h);
        self.map_all(|c| {
            let mut n = vec![CLEAR; nw * nh];
            for y in 0..h as i32 {
                for x in 0..w as i32 {
                    let (tx, ty) = (x + ox, y + oy);
                    if tx >= 0 && ty >= 0 && (tx as usize) < nw && (ty as usize) < nh {
                        n[ty as usize * nw + tx as usize] = c[y as usize * w + x as usize];
                    }
                }
            }
            n
        });
        self.w = nw;
        self.h = nh;
    }

    pub fn scale_canvas(&mut self, nw: usize, nh: usize) {
        let (w, h) = (self.w, self.h);
        self.map_all(|c| {
            let mut n = vec![CLEAR; nw * nh];
            for y in 0..nh {
                for x in 0..nw {
                    n[y * nw + x] = c[(y * h / nh) * w + (x * w / nw)];
                }
            }
            n
        });
        self.w = nw;
        self.h = nh;
    }
}

pub fn flip_buf(c: &[Px], w: usize, h: usize, horizontal: bool) -> Vec<Px> {
    let mut n = vec![CLEAR; w * h];
    for y in 0..h {
        for x in 0..w {
            let (tx, ty) = if horizontal { (w - 1 - x, y) } else { (x, h - 1 - y) };
            n[ty * w + tx] = c[y * w + x];
        }
    }
    n
}

// ---------- algorithms ----------

pub fn line_points(a: (i32, i32), b: (i32, i32)) -> Vec<(i32, i32)> {
    let (mut x0, mut y0) = a;
    let (x1, y1) = b;
    let dx = (x1 - x0).abs();
    let dy = -(y1 - y0).abs();
    let sx = if x0 < x1 { 1 } else { -1 };
    let sy = if y0 < y1 { 1 } else { -1 };
    let mut err = dx + dy;
    let mut pts = Vec::new();
    loop {
        pts.push((x0, y0));
        if x0 == x1 && y0 == y1 {
            break;
        }
        let e2 = 2 * err;
        if e2 >= dy {
            err += dy;
            x0 += sx;
        }
        if e2 <= dx {
            err += dx;
            y0 += sy;
        }
    }
    pts
}

fn close(p: Px, t: Px, tol: i32) -> bool {
    if p[3] == 0 && t[3] == 0 {
        return true;
    }
    (0..4).all(|i| (p[i] as i32 - t[i] as i32).abs() <= tol)
}

pub fn flood_mask(buf: &[Px], w: usize, h: usize, x: i32, y: i32, tol: i32, global: bool) -> Vec<bool> {
    let mut mask = vec![false; w * h];
    if x < 0 || y < 0 || x >= w as i32 || y >= h as i32 {
        return mask;
    }
    let target = buf[y as usize * w + x as usize];
    if global {
        for i in 0..w * h {
            mask[i] = close(buf[i], target, tol);
        }
        return mask;
    }
    let mut stack = vec![(x as usize, y as usize)];
    while let Some((cx, cy)) = stack.pop() {
        let i = cy * w + cx;
        if mask[i] || !close(buf[i], target, tol) {
            continue;
        }
        mask[i] = true;
        if cx > 0 {
            stack.push((cx - 1, cy));
        }
        if cx + 1 < w {
            stack.push((cx + 1, cy));
        }
        if cy > 0 {
            stack.push((cx, cy - 1));
        }
        if cy + 1 < h {
            stack.push((cx, cy + 1));
        }
    }
    mask
}

pub fn rect_mask(w: usize, h: usize, a: (i32, i32), b: (i32, i32)) -> Vec<bool> {
    let (x0, x1) = (a.0.min(b.0).max(0), a.0.max(b.0).min(w as i32 - 1));
    let (y0, y1) = (a.1.min(b.1).max(0), a.1.max(b.1).min(h as i32 - 1));
    let mut m = vec![false; w * h];
    for y in y0..=y1 {
        for x in x0..=x1 {
            m[y as usize * w + x as usize] = true;
        }
    }
    m
}

pub fn mask_bbox(mask: &[bool], w: usize) -> Option<(usize, usize, usize, usize)> {
    let (mut x0, mut y0, mut x1, mut y1) = (usize::MAX, usize::MAX, 0, 0);
    let mut any = false;
    for (i, &m) in mask.iter().enumerate() {
        if m {
            any = true;
            let (x, y) = (i % w, i / w);
            x0 = x0.min(x);
            y0 = y0.min(y);
            x1 = x1.max(x);
            y1 = y1.max(y);
        }
    }
    if any { Some((x0, y0, x1, y1)) } else { None }
}

// ---------- brush plotting ----------

#[derive(Clone, Copy)]
pub enum Op {
    Set(Px),
    Shade(f32),
}

pub struct PlotCtx<'a> {
    pub w: i32,
    pub h: i32,
    pub size: i32,
    pub round: bool,
    pub dither: bool,
    pub sx: bool,
    pub sy: bool,
    pub sel: Option<&'a [bool]>,
    pub density: f32,
    pub rng: std::cell::Cell<u64>,
}

impl<'a> PlotCtx<'a> {
    fn rnd(&self) -> f32 {
        let mut x = self.rng.get();
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.rng.set(x);
        (x >> 40) as f32 / (1u64 << 24) as f32
    }

    pub fn put(&self, buf: &mut [Px], x: i32, y: i32, op: Op) {
        if x < 0 || y < 0 || x >= self.w || y >= self.h {
            return;
        }
        let i = (y * self.w + x) as usize;
        if let Some(s) = self.sel {
            if !s[i] {
                return;
            }
        }
        if self.dither && (x + y) & 1 != 0 {
            return;
        }
        if self.density < 1.0 && self.rnd() > self.density {
            return;
        }
        match op {
            Op::Set(c) => buf[i] = c,
            Op::Shade(a) => {
                let p = &mut buf[i];
                if p[3] == 0 {
                    return;
                }
                for k in 0..3 {
                    let v = p[k] as f32;
                    p[k] = if a >= 0.0 { v + (255.0 - v) * a } else { v * (1.0 + a) }.round().clamp(0.0, 255.0) as u8;
                }
            }
        }
    }

    pub fn put_sym(&self, buf: &mut [Px], x: i32, y: i32, op: Op) {
        self.put(buf, x, y, op);
        if self.sx {
            self.put(buf, self.w - 1 - x, y, op);
        }
        if self.sy {
            self.put(buf, x, self.h - 1 - y, op);
        }
        if self.sx && self.sy {
            self.put(buf, self.w - 1 - x, self.h - 1 - y, op);
        }
    }

    pub fn brush_offsets(&self) -> Vec<(i32, i32)> {
        let s = self.size.max(1);
        let o = (s - 1) / 2;
        let c = (s - 1) as f32 / 2.0;
        let r = s as f32 / 2.0;
        let mut v = Vec::new();
        for dy in 0..s {
            for dx in 0..s {
                if self.round && s > 2 {
                    let d2 = (dx as f32 - c).powi(2) + (dy as f32 - c).powi(2);
                    if d2 > r * r - 0.5 {
                        continue;
                    }
                }
                v.push((dx - o, dy - o));
            }
        }
        v
    }

    pub fn put_brush(&self, buf: &mut [Px], x: i32, y: i32, op: Op) {
        for (dx, dy) in self.brush_offsets() {
            self.put_sym(buf, x + dx, y + dy, op);
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Shape {
    Line,
    Rect,
    Ellipse,
}

pub fn draw_shape(buf: &mut [Px], c: &PlotCtx, shape: Shape, a: (i32, i32), mut b: (i32, i32), fill: bool, col: Px, constrain: bool) {
    let op = Op::Set(col);
    if constrain {
        let (dx, dy) = (b.0 - a.0, b.1 - a.1);
        match shape {
            Shape::Line => {
                if dx.abs() > dy.abs() * 2 {
                    b.1 = a.1;
                } else if dy.abs() > dx.abs() * 2 {
                    b.0 = a.0;
                } else {
                    let m = dx.abs().max(dy.abs());
                    b = (a.0 + m * dx.signum(), a.1 + m * dy.signum());
                }
            }
            _ => {
                let m = dx.abs().max(dy.abs());
                b = (a.0 + m * if dx == 0 { 1 } else { dx.signum() }, a.1 + m * if dy == 0 { 1 } else { dy.signum() });
            }
        }
    }
    match shape {
        Shape::Line => {
            for (x, y) in line_points(a, b) {
                c.put_brush(buf, x, y, op);
            }
        }
        Shape::Rect => {
            let (x0, x1) = (a.0.min(b.0), a.0.max(b.0));
            let (y0, y1) = (a.1.min(b.1), a.1.max(b.1));
            if fill {
                for y in y0..=y1 {
                    for x in x0..=x1 {
                        c.put_sym(buf, x, y, op);
                    }
                }
            }
            for (p, q) in [((x0, y0), (x1, y0)), ((x1, y0), (x1, y1)), ((x1, y1), (x0, y1)), ((x0, y1), (x0, y0))] {
                for (x, y) in line_points(p, q) {
                    c.put_brush(buf, x, y, op);
                }
            }
        }
        Shape::Ellipse => {
            let (x0, x1) = (a.0.min(b.0), a.0.max(b.0));
            let (y0, y1) = (a.1.min(b.1), a.1.max(b.1));
            let cx = (x0 + x1) as f32 / 2.0;
            let cy = (y0 + y1) as f32 / 2.0;
            let rx = (x1 - x0) as f32 / 2.0 + 0.5;
            let ry = (y1 - y0) as f32 / 2.0 + 0.5;
            for y in y0..=y1 {
                let dy = (y as f32 - cy) / ry;
                let dxx = rx * (1.0 - dy * dy).max(0.0).sqrt();
                let (xl, xr) = ((cx - dxx + 0.01).round() as i32, (cx + dxx - 0.01).round() as i32);
                if fill {
                    for x in xl..=xr {
                        c.put_sym(buf, x, y, op);
                    }
                }
                c.put_brush(buf, xl, y, op);
                c.put_brush(buf, xr, y, op);
            }
            for x in x0..=x1 {
                let dx = (x as f32 - cx) / rx;
                let dyy = ry * (1.0 - dx * dx).max(0.0).sqrt();
                let (yt, yb) = ((cy - dyy + 0.01).round() as i32, (cy + dyy - 0.01).round() as i32);
                c.put_brush(buf, x, yt, op);
                c.put_brush(buf, x, yb, op);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rotate_roundtrip() {
        let mut d = Doc::new(5, 3);
        Arc::make_mut(&mut d.frames[0][0])[1] = [1, 2, 3, 255];
        let before = d.frames[0][0].to_vec();
        d.rotate_canvas(true);
        assert_eq!((d.w, d.h), (3, 5));
        d.rotate_canvas(false);
        assert_eq!(before, d.frames[0][0].to_vec());
    }

    #[test]
    fn flood_fill_contiguous() {
        let mut buf = vec![CLEAR; 16];
        for y in 0..4 {
            buf[y * 4 + 2] = [255, 0, 0, 255];
        }
        let m = flood_mask(&buf, 4, 4, 0, 0, 0, false);
        assert_eq!(m.iter().filter(|b| **b).count(), 8);
        let g = flood_mask(&buf, 4, 4, 0, 0, 0, true);
        assert_eq!(g.iter().filter(|b| **b).count(), 12);
    }

    #[test]
    fn layers_and_frames() {
        let mut d = Doc::new(4, 4);
        let l = d.add_layer(0);
        assert_eq!(l, 1);
        let f = d.add_frame(0);
        assert_eq!(d.frames[f].len(), 2);
        d.pingpong();
        d.delete_layer(1);
        assert!(d.frames.iter().all(|f| f.len() == 1));
    }

    #[test]
    fn project_roundtrip() {
        let d = Doc::new(8, 8);
        let s = serde_json::to_vec(&d).unwrap();
        let back: Doc = serde_json::from_slice(&s).unwrap();
        assert_eq!(back.w, 8);
        assert_eq!(back.frames[0][0].len(), 64);
    }

    #[test]
    fn ellipse_and_line_draw() {
        let mut buf = vec![CLEAR; 32 * 32];
        let c = PlotCtx { w: 32, h: 32, size: 1, round: false, dither: false, sx: false, sy: false, sel: None, density: 1.0, rng: std::cell::Cell::new(1) };
        draw_shape(&mut buf, &c, Shape::Ellipse, (4, 4), (27, 20), true, [9, 9, 9, 255], false);
        assert!(buf[12 * 32 + 16][3] == 255);
        assert!(buf[0][3] == 0);
        draw_shape(&mut buf, &c, Shape::Line, (0, 0), (31, 31), false, [1, 1, 1, 255], false);
        assert_eq!(buf[31 * 32 + 31], [1, 1, 1, 255]);
    }
}
