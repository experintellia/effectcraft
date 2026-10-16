//! File ▸ Export ▸ Telegram Sticker… through the wired session (real VP9 exporter): synthetic
//! comps built from commands, checked against Telegram's sticker limits.

use serde_json::{Value, json};

fn tmp(name: &str) -> String {
    let d = std::env::temp_dir().join(format!("effectcraft-host-telegram-{}", std::process::id()));
    std::fs::create_dir_all(&d).unwrap();
    d.join(name).to_string_lossy().to_string()
}

/// A shape-only sticker: a red ellipse moving across a transparent `size`² canvas.
fn sticker(size: u32, fps: u32, seconds: f64) -> effectcraft_engine::Session {
    let mut s = effectcraft_host::session();
    s.execute("comp.new", json!({"name": "Sticker", "width": size, "height": size, "frameRate": fps, "duration": seconds})).unwrap();
    s.execute("layer.newShape", json!({"kind": "ellipse", "size": [200, 200], "fill": "#e03030"})).unwrap();
    let c = size as f64 / 2.0;
    s.execute("prop.addKey", json!({"layer": "#1", "path": "transform/position", "time": 0.0, "value": [c - 100.0, c]})).unwrap();
    s.execute("prop.addKey", json!({"layer": "#1", "path": "transform/position", "time": seconds / 2.0, "value": [c + 100.0, c]})).unwrap();
    s
}

fn gunzip(data: &[u8]) -> Vec<u8> {
    assert_eq!(&data[..3], &[0x1f, 0x8b, 8], "gzip header");
    let body = &data[10..data.len() - 8];
    let out = miniz_oxide::inflate::decompress_to_vec(body).unwrap();
    let len = u32::from_le_bytes(data[data.len() - 4..].try_into().unwrap());
    assert_eq!(len as usize, out.len(), "ISIZE");
    out
}

/// The EBML header and track entries: everything before the first Cluster.
fn webm_head(data: &[u8]) -> &[u8] {
    let at = data.windows(4).position(|w| w == [0x1F, 0x43, 0xB6, 0x75]).unwrap_or(data.len());
    &data[..at]
}

fn has(hay: &[u8], needle: &[u8]) -> bool {
    hay.windows(needle.len()).any(|w| w == needle)
}

#[test]
fn a_shape_sticker_exports_as_tgs() {
    let mut s = sticker(512, 60, 3.0);
    let path = tmp("sticker.tgs");
    let r = s.execute_checked("file.exportTelegramSticker", json!({"path": path, "format": "tgs"})).unwrap();
    assert_eq!(r["problems"], json!([]), "{r}");
    let data = std::fs::read(&path).unwrap();
    assert_eq!(r["bytes"].as_u64(), Some(data.len() as u64));
    assert!(data.len() <= 64 * 1024, "{} bytes", data.len());
    let doc: Value = serde_json::from_slice(&gunzip(&data)).unwrap();
    assert_eq!((doc["tgs"].as_u64(), doc["w"].as_u64(), doc["h"].as_u64(), doc["fr"].as_f64()), (Some(1), Some(512), Some(512), Some(60.0)));
    assert_eq!(doc["op"].as_f64(), Some(180.0));
}

#[test]
fn a_30_fps_comp_is_written_at_60_fps_with_its_key_times() {
    let mut s = sticker(512, 30, 2.0);
    let path = tmp("thirty.tgs");
    s.execute("file.exportTelegramSticker", json!({"path": path})).unwrap();
    let doc: Value = serde_json::from_slice(&gunzip(&std::fs::read(&path).unwrap())).unwrap();
    assert_eq!((doc["fr"].as_f64(), doc["op"].as_f64()), (Some(60.0), Some(120.0)));
    let keys = doc["layers"][0]["ks"]["p"]["k"].as_array().unwrap();
    assert_eq!(keys[1]["t"].as_f64(), Some(60.0), "the 1 s key is frame 60");
}

#[test]
fn a_shape_sticker_exports_as_webm_with_alpha_and_no_audio() {
    let mut s = sticker(512, 60, 3.0);
    let path = tmp("sticker.webm");
    let r = s.execute_checked("file.exportTelegramSticker", json!({"path": path, "format": "webm"})).unwrap();
    assert_eq!(r["problems"], json!([]), "{r}");
    let data = std::fs::read(&path).unwrap();
    assert!(data.len() <= 256 * 1024, "{} bytes", data.len());
    let head = webm_head(&data);
    assert!(has(head, b"V_VP9"), "VP9 track");
    assert!(has(head, &[0x53, 0xC0, 0x81, 0x01]), "AlphaMode 1");
    assert!(!has(head, &[0x83, 0x81, 0x02]), "no audio track");
    assert!(!has(head, b"A_OPUS"), "no Opus track");
}

#[test]
fn stickers_breaking_telegram_limits_report_problems_and_write_nothing() {
    let png = tmp("dot.png");
    image::RgbaImage::from_pixel(8, 8, image::Rgba([0, 128, 255, 255])).save(&png).unwrap();
    let cases: Vec<(&str, &str, Box<dyn Fn(&mut effectcraft_engine::Session)>, &str)> = vec![
        ("long", "tgs", Box::new(|s| *s = sticker(512, 60, 4.0)), "at most 3 s"),
        ("long", "webm", Box::new(|s| *s = sticker(512, 60, 4.0)), "at most 3 s"),
        ("big", "tgs", Box::new(|s| *s = sticker(1000, 60, 2.0)), "must be 512×512"),
        (
            "expr",
            "tgs",
            Box::new(|s| {
                s.execute("prop.setExpression", json!({"layer": "#1", "path": "transform/rotation", "expression": "time * 90"})).unwrap();
            }),
            "expression",
        ),
        (
            "image",
            "tgs",
            Box::new(move |s| {
                s.execute("file.import", json!({"paths": [png.clone()], "addToComp": true})).unwrap();
            }),
            "images are not allowed",
        ),
    ];
    for (name, format, setup, want) in cases {
        let mut s = sticker(512, 60, 3.0);
        setup(&mut s);
        let path = tmp(&format!("{name}.{format}"));
        let _ = std::fs::remove_file(&path);
        // Dry run: the problems, nothing written.
        let r = s.execute("file.exportTelegramSticker", json!({"path": path, "format": format, "check": true})).unwrap();
        assert!(r["problems"].as_array().unwrap().iter().any(|p| p.as_str().unwrap().contains(want)), "{name}.{format}: {r}");
        let e = s.execute("file.exportTelegramSticker", json!({"path": path, "format": format})).unwrap_err().to_string();
        assert!(e.contains(want), "{name}.{format}: {e}");
        assert!(!std::path::Path::new(&path).exists(), "{name}.{format} was written");
        // `force` writes it anyway and still lists the problems.
        let r = s.execute("file.exportTelegramSticker", json!({"path": path, "format": format, "force": true})).unwrap();
        assert!(!r["problems"].as_array().unwrap().is_empty() && std::path::Path::new(&path).exists(), "{name}.{format}: {r}");
    }
}

#[test]
fn the_command_survives_empty_and_hostile_params() {
    let mut s = effectcraft_host::session();
    assert!(s.execute("file.exportTelegramSticker", json!({})).is_err(), "no comp");
    let mut s = sticker(512, 60, 1.0);
    for p in [json!({}), json!({"format": "gif", "path": "x"}), json!({"comp": 999, "path": "x"}), json!({"path": 5}), json!({"check": "yes"})] {
        assert!(s.execute("file.exportTelegramSticker", p.clone()).is_err(), "{p}");
    }
    // With nothing but `check` it reports on the active comp.
    let r = s.execute("file.exportTelegramSticker", json!({"check": true})).unwrap();
    assert_eq!((r["format"].as_str(), r["written"].as_bool()), (Some("tgs"), Some(false)), "{r}");
}
