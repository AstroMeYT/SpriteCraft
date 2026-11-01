//! Pixel-art effects: RotSprite rotation, outlines, drop shadows, auto-shading, tolerant color replace, lasso fill.
use crate::doc::{Px, CLEAR};
use eframe::egui::ecolor::Hsva;

fn same(a: Px, b: Px) -> bool {
    a == b || (a[3] == 0 && b[3] == 0)
}

/// EPX / Scale2x: doubles the image while keeping hard pixel edges.
pub fn scale2x(src: &[Px], w: usize, h: usize) -> Vec<Px> {
    let mut out = vec![CLEAR; 4 * w * h];
    let get = |x: i32, y: i32| -> Px {
        let cx = x.clamp(0, w as i32 - 1) as usize;
        let cy = y.clamp(0, h as i32 - 1) as usize;
        src[cy * w + cx]
    };
    for y in 0..h as i32 {
        for x in 0..w as i32 {
            let p = get(x, y);
            let a = get(x, y - 1);
            let b = get(x + 1, y);
            let c = get(x - 1, y);
            let d = get(x, y + 1);
            let (mut e0, mut e1, mut e2, mut e3) = (p, p, p, p);
            if same(c, a) && !same(c, d) && !same(a, b) {
                e0 = a;
            }
            if same(a, b) && !same(a, c) && !same(b, d) {
                e1 = b;
            }
            if same(d, c) && !same(d, b) && !same(c, a) {
                e2 = c;
            }
            if same(b, d) && !same(b, a) && !same(d, c) {
                e3 = d;
            }
            let (ox, oy) = (x as usize * 2, y as usize * 2);
            let ow = w * 2;
            out[oy * ow + ox] = e0;
            out[oy * ow + ox + 1] = e1;
            out[(oy + 1) * ow + ox] = e2;
            out[(oy + 1) * ow + ox + 1] = e3;
        }
    }
    out
}

/// RotSprite: scale up 8x with Scale2x, rotate with nearest neighbour, sample back down.
/// Positive angles rotate clockwise. (cx, cy) is the pivot in pixel coordinates.
pub fn rotsprite(buf: &[Px], w: usize, h: usize, angle_deg: f32, cx: f32, cy: f32) -> Vec<Px> {
    let mut img = buf.to_vec();
    let (mut cw, mut ch) = (w, h);
    for _ in 0..3 {
        img = scale2x(&img, cw, ch);
        cw *= 2;
        ch *= 2;
    }
    let s = 8.0f32;
    let (sin, cos) = angle_deg.to_radians().sin_cos();
    let mut out = vec![CLEAR; w * h];
    for y in 0..h {
        for x in 0..w {
            let px = (x as f32 + 0.5 - cx) * s;
            let py = (y as f32 + 0.5 - cy) * s;
            let sx = px * cos + py * sin + cx * s;
            let sy = -px * sin + py * cos + cy * s;
            let (ix, iy) = (sx.floor() as i32, sy.floor() as i32);
            if ix >= 0 && iy >= 0 && (ix as usize) < cw && (iy as usize) < ch {
                out[y * w + x] = img[iy as usize * cw + ix as usize];
            }
        }
    }
    out
}

fn neighbours(diag: bool) -> &'static [(i32, i32)] {
    if diag {
        &[(-1, 0), (1, 0), (0, -1), (0, 1), (-1, -1), (1, -1), (-1, 1), (1, 1)]
    } else {
        &[(-1, 0), (1, 0), (0, -1), (0, 1)]
    }
}

fn at(buf: &[Px], w: usize, h: usize, x: i32, y: i32) -> Option<Px> {
    if x < 0 || y < 0 || x >= w as i32 || y >= h as i32 {
        None
    } else {
        Some(buf[y as usize * w + x as usize])
    }
}

/// Outer outline grows outwards; inner outline recolors the sprite's own edge pixels.
pub fn outline(buf: &mut [Px], w: usize, h: usize, col: Px, thickness: usize, diag: bool, inner: bool) {
    for _ in 0..thickness.max(1) {
        let src = buf.to_vec();
        for y in 0..h as i32 {
            for x in 0..w as i32 {
                let p = src[y as usize * w + x as usize];
                if inner {
                    if p[3] == 0 {
                        continue;
                    }
                    let edge = neighbours(diag).iter().any(|&(dx, dy)| at(&src, w, h, x + dx, y + dy).map_or(true, |q| q[3] == 0));
                    if edge {
                        buf[y as usize * w + x as usize] = col;
                    }
                } else {
                    if p[3] != 0 {
                        continue;
                    }
                    let near = neighbours(diag).iter().any(|&(dx, dy)| at(&src, w, h, x + dx, y + dy).map_or(false, |q| q[3] != 0));
                    if near {
                        buf[y as usize * w + x as usize] = col;
                    }
                }
            }
        }
    }
}

pub fn drop_shadow(buf: &mut [Px], w: usize, h: usize, dx: i32, dy: i32, col: Px) {
    let src = buf.to_vec();
    for y in 0..h as i32 {
        for x in 0..w as i32 {
            if src[y as usize * w + x as usize][3] == 0 {
                continue;
            }
            let (tx, ty) = (x + dx, y + dy);
            if tx >= 0 && ty >= 0 && tx < w as i32 && ty < h as i32 {
                let i = ty as usize * w + tx as usize;
                if src[i][3] == 0 {
                    buf[i] = col;
                }
            }
        }
    }
}

fn hue_toward(h: f32, target: f32, f: f32) -> f32 {
    let delta = ((target - h + 0.5).rem_euclid(1.0)) - 0.5;
    (h + delta * f).rem_euclid(1.0)
}

/// Adds highlights on edges facing the light and shadows on the opposite edges.
/// `light` points from the sprite toward the light (e.g. (-1,-1) = top-left).
pub fn auto_shade(buf: &mut [Px], w: usize, h: usize, light: (i32, i32), depth: usize, strength: f32, hue_shift: f32) {
    let src = buf.to_vec();
    let depth = depth.max(1);
    let open = |x: i32, y: i32| at(&src, w, h, x, y).map_or(true, |q| q[3] == 0);
    for y in 0..h as i32 {
        for x in 0..w as i32 {
            let p = src[y as usize * w + x as usize];
            if p[3] == 0 {
                continue;
            }
            let (mut hi, mut lo) = (0.0f32, 0.0f32);
            for k in 1..=depth as i32 {
                let amt = (depth as i32 - k + 1) as f32 / depth as f32;
                if hi == 0.0 && open(x + light.0 * k, y + light.1 * k) {
                    hi = amt;
                }
                if lo == 0.0 && open(x - light.0 * k, y - light.1 * k) {
                    lo = amt;
                }
            }
            if hi == 0.0 && lo == 0.0 {
                continue;
            }
            let mut c = Hsva::from_srgba_unmultiplied(p);
            if hi >= lo {
                let a = hi * strength;
                c.h = hue_toward(c.h, 60.0 / 360.0, hue_shift * a);
                c.v = (c.v + (1.0 - c.v) * a).clamp(0.0, 1.0);
                c.s = (c.s * (1.0 - a * 0.35)).clamp(0.0, 1.0);
            } else {
                let a = lo * strength;
                c.h = hue_toward(c.h, 240.0 / 360.0, hue_shift * a);
                c.v = (c.v * (1.0 - a * 0.6)).clamp(0.0, 1.0);
                c.s = (c.s * (1.0 + a * 0.25)).clamp(0.0, 1.0);
            }
            let alpha = p[3];
            let mut o = c.to_srgba_unmultiplied();
            o[3] = alpha;
            buf[y as usize * w + x as usize] = o;
        }
    }
}

/// True when every channel of `p` is within `tol` of `from`. Fully transparent pixels only match a transparent source.
pub fn color_close(p: Px, from: Px, tol: i32) -> bool {
    if p[3] == 0 || from[3] == 0 {
        return p[3] == 0 && from[3] == 0;
    }
    (0..4).all(|i| (p[i] as i32 - from[i] as i32).abs() <= tol)
}

pub fn replace_color(buf: &mut [Px], from: Px, to: Px, tol: i32) -> usize {
    let mut n = 0;
    for p in buf.iter_mut() {
        if color_close(*p, from, tol) {
            *p = to;
            n += 1;
        }
    }
    n
}

/// Even-odd scanline fill of a polygon (pixel centres), used by the lasso tool.
pub fn polygon_mask(w: usize, h: usize, pts: &[(i32, i32)]) -> Vec<bool> {
    let mut m = vec![false; w * h];
    if pts.len() < 3 {
        return m;
    }
    for y in 0..h {
        let yc = y as f32 + 0.5;
        let mut xs: Vec<f32> = vec![];
        for i in 0..pts.len() {
            let a = (pts[i].0 as f32 + 0.5, pts[i].1 as f32 + 0.5);
            let b = (pts[(i + 1) % pts.len()].0 as f32 + 0.5, pts[(i + 1) % pts.len()].1 as f32 + 0.5);
            if (a.1 <= yc && b.1 > yc) || (b.1 <= yc && a.1 > yc) {
                xs.push(a.0 + (yc - a.1) / (b.1 - a.1) * (b.0 - a.0));
            }
        }
        xs.sort_by(|a, b| a.partial_cmp(b).unwrap());
        for pair in xs.chunks(2) {
            if pair.len() == 2 {
                for x in 0..w {
                    let xc = x as f32 + 0.5;
                    if xc >= pair[0] && xc <= pair[1] {
                        m[y * w + x] = true;
                    }
                }
            }
        }
    }
    // always include the traced outline itself so thin lassos still select something
    for &(x, y) in pts {
        if x >= 0 && y >= 0 && (x as usize) < w && (y as usize) < h {
            m[y as usize * w + x as usize] = true;
        }
    }
    m
}

#[cfg(test)]
mod tests {
    use super::*;

    const R: Px = [255, 0, 0, 255];

    #[test]
    fn rotsprite_zero_is_identity() {
        let (w, h) = (8, 8);
        let mut b = vec![CLEAR; w * h];
        for y in 2..6 {
            for x in 1..5 {
                b[y * w + x] = R;
            }
        }
        let r = rotsprite(&b, w, h, 0.0, w as f32 / 2.0, h as f32 / 2.0);
        assert_eq!(b, r);
    }

    #[test]
    fn rotsprite_90_matches_rotation() {
        let (w, h) = (6, 6);
        let mut b = vec![CLEAR; w * h];
        b[1 * w + 4] = R;
        let r = rotsprite(&b, w, h, 90.0, 3.0, 3.0);
        // (4,1) rotated 90 degrees clockwise about (3,3) lands at (5,4)
        assert_eq!(r[4 * w + 4], R);
        assert_eq!(r.iter().filter(|p| p[3] > 0).count(), 1);
    }

    #[test]
    fn outline_grows_and_shadow_offsets() {
        let (w, h) = (5, 5);
        let mut b = vec![CLEAR; w * h];
        b[2 * w + 2] = R;
        let mut o = b.clone();
        outline(&mut o, w, h, [0, 0, 0, 255], 1, false, false);
        assert_eq!(o.iter().filter(|p| p[3] > 0).count(), 5);
        let mut o8 = b.clone();
        outline(&mut o8, w, h, [0, 0, 0, 255], 1, true, false);
        assert_eq!(o8.iter().filter(|p| p[3] > 0).count(), 9);
        let mut s = b.clone();
        drop_shadow(&mut s, w, h, 1, 1, [0, 0, 0, 255]);
        assert_eq!(s[3 * w + 3], [0, 0, 0, 255]);
    }

    #[test]
    fn replace_with_tolerance() {
        let mut b = vec![[250, 0, 0, 255], [255, 5, 0, 255], [0, 0, 255, 255], CLEAR];
        let n = replace_color(&mut b, R, [0, 255, 0, 255], 10);
        assert_eq!(n, 2);
        assert_eq!(b[2], [0, 0, 255, 255]);
        assert_eq!(replace_color(&mut b, R, [1, 1, 1, 255], 0), 0);
    }

    #[test]
    fn lasso_fills_triangle() {
        let m = polygon_mask(10, 10, &[(1, 1), (8, 1), (1, 8)]);
        assert!(m[2 * 10 + 2]);
        assert!(!m[8 * 10 + 8]);
    }

    #[test]
    fn auto_shade_changes_edges_only() {
        let (w, h) = (7, 7);
        let mut b = vec![CLEAR; w * h];
        for y in 1..6 {
            for x in 1..6 {
                b[y * w + x] = [120, 120, 120, 255];
            }
        }
        let before = b.clone();
        auto_shade(&mut b, w, h, (-1, -1), 2, 0.6, 0.5);
        assert_ne!(before[1 * w + 1], b[1 * w + 1]);
        assert_eq!(before[3 * w + 3], b[3 * w + 3]);
    }
}
