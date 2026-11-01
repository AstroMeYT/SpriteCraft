//! Game-engine oriented exports: Godot 4 SpriteFrames, Aseprite-style JSON, strips and CSS snippets.
use crate::doc::*;
use crate::{sanitize_ext, scaled_image};
use std::path::PathBuf;

pub struct Opts<'a> {
    pub dir: &'a str,
    pub name: &'a str,
    pub scale: usize,
    pub cols: usize,
}

pub fn frame_label(doc: &Doc, i: usize, stem: &str) -> String {
    match doc.names.get(i) {
        Some(n) if !n.trim().is_empty() => n.clone(),
        _ => format!("{stem} {i}"),
    }
}

fn duration_ms(doc: &Doc, i: usize) -> u32 {
    doc.holds[i] * 1000 / doc.fps.max(1)
}

fn build_sheet(doc: &Doc, cols: usize, s: usize) -> (image::RgbaImage, Vec<(usize, usize)>) {
    let n = doc.frames.len();
    let cols = cols.clamp(1, n);
    let rows = (n + cols - 1) / cols;
    let (fw, fh) = (doc.w * s, doc.h * s);
    let mut sheet = image::RgbaImage::new((cols * fw) as u32, (rows * fh) as u32);
    let mut pos = vec![];
    for i in 0..n {
        let img = scaled_image(&doc.composite(i), doc.w, doc.h, s);
        let (ox, oy) = ((i % cols) * fw, (i / cols) * fh);
        image::imageops::replace(&mut sheet, &img, ox as i64, oy as i64);
        pos.push((ox, oy));
    }
    (sheet, pos)
}

fn dir_name(d: &TagDir) -> &'static str {
    match d {
        TagDir::Forward => "forward",
        TagDir::Reverse => "reverse",
        TagDir::PingPong => "pingpong",
    }
}

fn esc(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

pub fn export_game(doc: &Doc, kind: usize, o: &Opts) -> Result<String, String> {
    let dir = PathBuf::from(o.dir);
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let stem = o.name.trim_end_matches(".png").trim_end_matches(".json").trim_end_matches(".tres").trim_end_matches(".html").to_string();
    let s = o.scale.max(1);
    let (fw, fh) = (doc.w * s, doc.h * s);
    let n = doc.frames.len();
    match kind {
        // Godot 4 SpriteFrames
        4 => {
            let (sheet, pos) = build_sheet(doc, o.cols, s);
            let png_name = sanitize_ext(&stem, "png");
            sheet.save(dir.join(&png_name)).map_err(|e| e.to_string())?;
            let mut t = String::new();
            t.push_str(&format!("[gd_resource type=\"SpriteFrames\" load_steps={} format=3]\n\n", n + 2));
            t.push_str(&format!("[ext_resource type=\"Texture2D\" path=\"res://{png_name}\" id=\"1\"]\n\n"));
            for (i, (x, y)) in pos.iter().enumerate() {
                t.push_str(&format!("[sub_resource type=\"AtlasTexture\" id=\"AtlasTexture_{i}\"]\natlas = ExtResource(\"1\")\nregion = Rect2({x}, {y}, {fw}, {fh})\n\n"));
            }
            let mut anims: Vec<(String, Vec<usize>)> = vec![("default".into(), (0..n).collect())];
            for tg in &doc.tags {
                anims.push((tg.name.clone(), doc.tag_sequence(tg)));
            }
            t.push_str("[resource]\nanimations = [");
            for (ai, (name, seq)) in anims.iter().enumerate() {
                if ai > 0 {
                    t.push_str(", ");
                }
                t.push_str("{\n\"frames\": [");
                for (k, &f) in seq.iter().enumerate() {
                    if k > 0 {
                        t.push_str(", ");
                    }
                    t.push_str(&format!("{{\n\"duration\": {:.1},\n\"texture\": SubResource(\"AtlasTexture_{f}\")\n}}", doc.holds[f] as f32));
                }
                t.push_str(&format!("],\n\"loop\": true,\n\"name\": &\"{}\",\n\"speed\": {:.1}\n}}", esc(name), doc.fps as f32));
            }
            t.push_str("]\n");
            let p = dir.join(format!("{stem}.tres"));
            std::fs::write(&p, t).map_err(|e| e.to_string())?;
            Ok(format!("{} (+ {png_name}; copy both into your Godot project root)", p.display()))
        }
        // Aseprite-style JSON (array format) + sheet
        5 => {
            let (sheet, pos) = build_sheet(doc, o.cols, s);
            let png_name = sanitize_ext(&stem, "png");
            sheet.save(dir.join(&png_name)).map_err(|e| e.to_string())?;
            let frames: Vec<_> = pos
                .iter()
                .enumerate()
                .map(|(i, (x, y))| {
                    serde_json::json!({
                        "filename": frame_label(doc, i, &stem),
                        "frame": {"x": x, "y": y, "w": fw, "h": fh},
                        "rotated": false, "trimmed": false,
                        "spriteSourceSize": {"x": 0, "y": 0, "w": fw, "h": fh},
                        "sourceSize": {"w": fw, "h": fh},
                        "duration": duration_ms(doc, i)
                    })
                })
                .collect();
            let tags: Vec<_> = doc.tags.iter().map(|t| serde_json::json!({"name": t.name, "from": t.from, "to": t.to, "direction": dir_name(&t.dir)})).collect();
            let meta = serde_json::json!({
                "app": "SpriteCraft", "version": env!("CARGO_PKG_VERSION"), "image": png_name, "format": "RGBA8888",
                "size": {"w": sheet.width(), "h": sheet.height()}, "scale": "1", "frameTags": tags, "slices": []
            });
            let p = dir.join(format!("{stem}.json"));
            std::fs::write(&p, serde_json::to_string_pretty(&serde_json::json!({"frames": frames, "meta": meta})).unwrap()).map_err(|e| e.to_string())?;
            Ok(format!("{} (+ {png_name})", p.display()))
        }
        // Horizontal strip
        6 => {
            let (sheet, _) = build_sheet(doc, n, s);
            let p = dir.join(sanitize_ext(&stem, "png"));
            sheet.save(&p).map_err(|e| e.to_string())?;
            Ok(format!("{} ({n} frames of {fw}x{fh})", p.display()))
        }
        // CSS / HTML snippet
        _ => {
            let (sheet, _) = build_sheet(doc, n, s);
            let png_name = sanitize_ext(&stem, "png");
            sheet.save(dir.join(&png_name)).map_err(|e| e.to_string())?;
            let total: u32 = (0..n).map(|i| duration_ms(doc, i)).sum::<u32>().max(1);
            let mut acc = 0u32;
            let mut kf = String::new();
            for i in 0..n {
                kf.push_str(&format!("  {:.3}% {{ background-position: -{}px 0; }}\n", acc as f32 / total as f32 * 100.0, i * fw));
                acc += duration_ms(doc, i);
            }
            let html = format!(
                "<!doctype html>\n<meta charset=\"utf-8\">\n<title>{stem}</title>\n<style>\n  body {{ background: #222; display: grid; place-items: center; height: 100vh; margin: 0; }}\n  .sprite {{\n    width: {fw}px; height: {fh}px;\n    background: url('{png_name}') 0 0 no-repeat;\n    image-rendering: pixelated;\n    animation: play {:.3}s step-end infinite;\n  }}\n  @keyframes play {{\n{kf}  }}\n</style>\n<div class=\"sprite\"></div>\n",
                total as f32 / 1000.0
            );
            let p = dir.join(format!("{stem}.html"));
            std::fs::write(&p, html).map_err(|e| e.to_string())?;
            Ok(format!("{} (+ {png_name})", p.display()))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    fn demo_doc() -> Doc {
        let mut d = Doc::new(4, 4);
        Arc::make_mut(&mut d.frames[0][0])[5] = [255, 0, 0, 255];
        d.add_frame(0);
        d.add_frame(1);
        d.names[1] = "mid".into();
        d.tags.push(Tag { name: "walk".into(), from: 0, to: 2, dir: TagDir::PingPong, color: [1, 2, 3] });
        d
    }

    fn tmp() -> String {
        let p = std::env::temp_dir().join(format!("spritecraft-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&p);
        p.to_string_lossy().to_string()
    }

    #[test]
    fn godot_json_strip_css() {
        let d = demo_doc();
        let dir = tmp();
        let o = Opts { dir: &dir, name: "hero", scale: 2, cols: 3 };
        export_game(&d, 4, &o).unwrap();
        let tres = std::fs::read_to_string(format!("{dir}/hero.tres")).unwrap();
        assert!(tres.contains("type=\"SpriteFrames\""));
        assert!(tres.contains("Rect2(8, 0, 8, 8)"));
        assert!(tres.contains("&\"walk\""));
        assert!(tres.contains("&\"default\""));
        export_game(&d, 5, &o).unwrap();
        let j: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(format!("{dir}/hero.json")).unwrap()).unwrap();
        assert_eq!(j["frames"].as_array().unwrap().len(), 3);
        assert_eq!(j["frames"][1]["filename"], "mid");
        assert_eq!(j["meta"]["frameTags"][0]["direction"], "pingpong");
        export_game(&d, 6, &o).unwrap();
        let strip = image::open(format!("{dir}/hero.png")).unwrap();
        assert_eq!(strip.width(), 3 * 8);
        export_game(&d, 7, &o).unwrap();
        let html = std::fs::read_to_string(format!("{dir}/hero.html")).unwrap();
        assert!(html.contains("@keyframes play"));
        let _ = std::fs::remove_dir_all(dir);
    }
}
