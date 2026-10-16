//! Telegram animated stickers (`.tgs`): a gzipped Lottie document with `"tgs": 1` at the root.
//!
//! Limits and the forbidden After Effects features come from Telegram's public sticker
//! documentation (core.telegram.org/stickers, "Animated Sticker Requirements").

use serde_json::Value as Json;

use crate::ExportResult;

/// Largest `.tgs` Telegram accepts.
pub const TGS_MAX_BYTES: usize = 64 * 1024;

/// Exporter warnings about features Lottie drops that Telegram forbids anyway (the sticker would
/// silently differ from the composition).
const FORBIDDEN_WARNINGS: &[&str] =
    &["expression not exported", "effect not supported", "cannot read image", "footage is not part of Lottie", "3D model", "cameras and lights"];

/// The `.tgs` file of an export: `"tgs": 1` added at the root, gzip (RFC 1952) of the compact JSON.
pub fn to_tgs(res: &ExportResult) -> Vec<u8> {
    let mut doc = res.json.clone();
    if let Some(o) = doc.as_object_mut() {
        o.insert("tgs".into(), Json::from(1));
    }
    let data = serde_json::to_vec(&doc).unwrap_or_default();
    // Header: magic, deflate, no flags, mtime 0, no extra flags, OS unknown.
    let mut out = vec![0x1f, 0x8b, 8, 0, 0, 0, 0, 0, 0, 0xff];
    out.extend(miniz_oxide::deflate::compress_to_vec(&data, 10));
    out.extend(crate::zip::crc32(&data).to_le_bytes());
    out.extend((data.len() as u32).to_le_bytes());
    out
}

/// What keeps an export from being a valid Telegram sticker: features Telegram forbids, found in
/// the document or in the exporter's warnings about what it dropped. Canvas size, length and file
/// size are the caller's (they depend on sticker vs emoji).
pub fn tgs_problems(res: &ExportResult) -> Vec<String> {
    let mut out: Vec<String> = res.warnings.iter().filter(|w| FORBIDDEN_WARNINGS.iter().any(|f| w.contains(f))).cloned().collect();
    let empty = vec![];
    let mut layer_lists = vec![res.json.get("layers").and_then(Json::as_array).unwrap_or(&empty)];
    for a in res.json.get("assets").and_then(Json::as_array).unwrap_or(&empty) {
        if let Some(l) = a.get("layers").and_then(Json::as_array) {
            layer_lists.push(l);
        }
    }
    for l in layer_lists.into_iter().flatten() {
        let name = l.get("nm").and_then(Json::as_str).unwrap_or("Layer");
        let num = |k: &str| l.get(k).and_then(Json::as_f64);
        let mut bad = |what: &str| out.push(format!("{name}: {what} are not allowed in Telegram stickers"));
        match num("ty") {
            Some(1.0) => bad("solid layers (use a shape layer)"),
            Some(2.0) => bad("images"),
            Some(5.0) => bad("text layers"),
            _ => {}
        }
        if num("ddd") == Some(1.0) {
            bad("3D layers");
        }
        if l.get("masksProperties").and_then(Json::as_array).is_some_and(|m| !m.is_empty()) {
            bad("masks");
        }
        if l.get("ef").and_then(Json::as_array).is_some_and(|m| !m.is_empty()) {
            bad("layer effects");
        }
        if l.get("tm").is_some() {
            bad("time remapping");
        }
        if num("sr").is_some_and(|s| (s - 1.0).abs() > 1e-9) {
            bad("time stretching");
        }
        if num("ao") == Some(1.0) {
            bad("auto-oriented layers");
        }
        let mut found = vec![];
        shape_problems(l.get("shapes"), &mut found);
        found.sort_unstable();
        found.dedup();
        for f in found {
            bad(f);
        }
    }
    out
}

fn shape_problems(items: Option<&Json>, found: &mut Vec<&'static str>) {
    for it in items.and_then(Json::as_array).into_iter().flatten() {
        match it.get("ty").and_then(Json::as_str) {
            Some("mm") => found.push("merge paths"),
            Some("rp") => found.push("repeaters"),
            Some("gs") => found.push("gradient strokes"),
            Some("sr") if it.get("sy").and_then(Json::as_f64) == Some(1.0) => found.push("star shapes"),
            _ => {}
        }
        shape_problems(it.get("it"), found);
    }
}
