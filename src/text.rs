//! Text rasterization: type text in a font, pixelate it into sprite pixels.
use crate::doc::Px;
use ab_glyph::{point, Font, FontVec, PxScale, ScaleFont};
use std::path::{Path, PathBuf};

pub const BUILTIN: &[(&str, &str)] = &[("Hack", "Hack"), ("Ubuntu Light", "Ubuntu-Light")];

pub fn scan_fonts() -> Vec<(String, Option<PathBuf>)> {
    let mut out: Vec<(String, Option<PathBuf>)> = BUILTIN.iter().map(|(n, _)| (n.to_string(), None)).collect();
    let mut dirs = vec![PathBuf::from("/usr/share/fonts"), PathBuf::from("/usr/local/share/fonts")];
    if let Some(h) = std::env::var_os("HOME") {
        let h = PathBuf::from(h);
        dirs.push(h.join(".local/share/fonts"));
        dirs.push(h.join(".fonts"));
    }
    let mut found: Vec<(String, PathBuf)> = vec![];
    for d in dirs {
        walk(&d, 0, &mut found);
    }
    found.sort_by(|a, b| a.0.to_lowercase().cmp(&b.0.to_lowercase()));
    found.dedup_by(|a, b| a.0 == b.0);
    out.extend(found.into_iter().take(1500).map(|(n, p)| (n, Some(p))));
    out
}

fn walk(dir: &Path, depth: usize, out: &mut Vec<(String, PathBuf)>) {
    if depth > 6 || out.len() > 4000 {
        return;
    }
    let Ok(rd) = std::fs::read_dir(dir) else { return };
    for e in rd.flatten() {
        let p = e.path();
        if p.is_dir() {
            walk(&p, depth + 1, out);
        } else if let Some(ext) = p.extension().and_then(|x| x.to_str()) {
            let ext = ext.to_lowercase();
            if ext == "ttf" || ext == "otf" {
                if let Some(stem) = p.file_stem().and_then(|s| s.to_str()) {
                    out.push((stem.to_string(), p.clone()));
                }
            }
        }
    }
}

pub fn load_font(name: &str, fonts: &[(String, Option<PathBuf>)]) -> Option<FontVec> {
    let bytes = match fonts.iter().find(|f| f.0 == name)?.1.clone() {
        Some(p) => std::fs::read(p).ok()?,
        None => {
            let key = BUILTIN.iter().find(|b| b.0 == name)?.1;
            eframe::egui::FontDefinitions::default().font_data.get(key)?.font.to_vec()
        }
    };
    FontVec::try_from_vec(bytes).ok()
}

/// Rasterize `text` at `size` px, then threshold (or keep smooth alpha) to get crisp sprite pixels.
pub fn render_text(font: &FontVec, text: &str, size: f32, smooth: bool, thresh: f32, col: Px) -> Option<(usize, usize, Vec<Px>)> {
    if text.trim().is_empty() {
        return None;
    }
    let scale = PxScale::from(size);
    let sf = font.as_scaled(scale);
    let line_h = (sf.ascent() - sf.descent() + sf.line_gap()).ceil().max(1.0);
    let lines: Vec<&str> = text.split('\n').collect();
    let mut width = 0f32;
    for l in &lines {
        let (mut x, mut prev) = (0f32, None);
        for c in l.chars() {
            let id = font.glyph_id(c);
            if let Some(p) = prev {
                x += sf.kern(p, id);
            }
            x += sf.h_advance(id);
            prev = Some(id);
        }
        width = width.max(x);
    }
    let w = width.ceil() as usize + 4;
    let h = (line_h * lines.len() as f32).ceil() as usize + 4;
    if w > 2048 || h > 2048 {
        return None;
    }
    let mut cov = vec![0f32; w * h];
    for (li, l) in lines.iter().enumerate() {
        let base_y = 2.0 + li as f32 * line_h + sf.ascent();
        let (mut x, mut prev) = (2.0f32, None);
        for c in l.chars() {
            let id = font.glyph_id(c);
            if let Some(p) = prev {
                x += sf.kern(p, id);
            }
            let g = id.with_scale_and_position(scale, point(x, base_y));
            x += sf.h_advance(id);
            prev = Some(id);
            if let Some(og) = font.outline_glyph(g) {
                let b = og.px_bounds();
                og.draw(|gx, gy, cv| {
                    let px = b.min.x as i32 + gx as i32;
                    let py = b.min.y as i32 + gy as i32;
                    if px >= 0 && py >= 0 && (px as usize) < w && (py as usize) < h {
                        let i = py as usize * w + px as usize;
                        cov[i] = cov[i].max(cv);
                    }
                });
            }
        }
    }
    let mut px = vec![[0u8; 4]; w * h];
    let (mut x0, mut y0, mut x1, mut y1) = (usize::MAX, usize::MAX, 0usize, 0usize);
    for y in 0..h {
        for x in 0..w {
            let c = cov[y * w + x];
            let a = if smooth { if c > 0.04 { (c * col[3] as f32) as u8 } else { 0 } } else if c >= thresh { col[3] } else { 0 };
            if a > 0 {
                px[y * w + x] = [col[0], col[1], col[2], a];
                x0 = x0.min(x);
                y0 = y0.min(y);
                x1 = x1.max(x);
                y1 = y1.max(y);
            }
        }
    }
    if x0 == usize::MAX {
        return None;
    }
    let (cw, ch) = (x1 - x0 + 1, y1 - y0 + 1);
    let mut out = Vec::with_capacity(cw * ch);
    for y in y0..=y1 {
        for x in x0..=x1 {
            out.push(px[y * w + x]);
        }
    }
    Some((cw, ch, out))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_builtin_font() {
        let fonts = scan_fonts();
        let f = load_font("Hack", &fonts).expect("builtin font");
        let (w, h, px) = render_text(&f, "Hi\nYo", 12.0, false, 0.5, [255, 0, 0, 255]).expect("pixels");
        assert!(w > 4 && h > 8);
        assert!(px.iter().any(|p| p[3] == 255));
        assert!(px.iter().all(|p| p[3] == 0 || p[3] == 255));
    }
}
