//! File ▸ Export ▸ Telegram Sticker…: a composition as a Telegram animated sticker (`.tgs`,
//! gzipped Lottie) or video sticker (`.webm`, VP9 with alpha), checked against Telegram's public
//! sticker requirements (core.telegram.org/stickers).
//!
//! Problems are hard Telegram limits; unless `force` is set, nothing is written while there are
//! any. Frame rates are conformed rather than refused: `.tgs` is written at 60 fps (keyframes keep
//! their times), `.webm` at the comp's rate capped at 30 fps.

use effectcraft_lottie::{ExportOptions, TGS_MAX_BYTES};
use effectcraft_project::render_queue::{RenderQueueItem, TimeSpan};
use effectcraft_project::render_templates::{
    TELEGRAM_MAX_SECONDS, TELEGRAM_STICKER_SIZE, TELEGRAM_VIDEO_FPS, TELEGRAM_VIDEO_MAX_BYTES, telegram_video_kbps, telegram_video_sticker,
};
use effectcraft_project::{Comp, ItemId};
use effectcraft_time::FrameRate;
use serde_json::{Value, json};

use super::{CommandSpec, b_p, bad, comp_id, has_comp, str_p};
use crate::render_queue::ExportJob;
use crate::{EngineError, Result, Session, cmd};

const ID: &str = "file.exportTelegramSticker";
/// Custom emoji are 100×100.
const EMOJI_SIZE: u32 = 100;
/// `.tgs` frame rate.
const TGS_FPS: i64 = 60;
/// Encodes of an oversized `.webm` before giving up.
const WEBM_TRIES: usize = 3;

fn kb(bytes: u64) -> String {
    format!("{:.1} KB", bytes as f64 / 1024.0)
}

/// Limits that hold before anything is rendered: canvas, length.
fn precheck(comp: &Comp, webm: bool, emoji: bool) -> Vec<String> {
    let mut out = vec![];
    let side = if emoji { EMOJI_SIZE } else { TELEGRAM_STICKER_SIZE };
    if !webm && (comp.width != side || comp.height != side) {
        out.push(format!("canvas is {}×{}; animated {} must be {side}×{side}", comp.width, comp.height, if emoji { "emoji" } else { "stickers" }));
    }
    if webm && emoji && comp.width != comp.height {
        out.push(format!("canvas is {}×{}; video emoji must be square ({side}×{side})", comp.width, comp.height));
    }
    // A .tgs holds the whole comp; a .webm renders the work area (like the Render Queue).
    let (a, b) = if webm { effectcraft_project::render_queue::RenderSettings::default().span(comp) } else { (Default::default(), comp.duration) };
    let secs = (b - a).seconds();
    if secs > TELEGRAM_MAX_SECONDS + 1e-6 {
        let what = if webm { "work area" } else { "composition" };
        out.push(format!("{what} is {secs:.2} s long; stickers may last at most {TELEGRAM_MAX_SECONDS} s"));
    }
    out
}

fn refuse(problems: &[String]) -> EngineError {
    EngineError::Other(format!("not a valid Telegram sticker (pass `force` to export anyway):\n- {}", problems.join("\n- ")))
}

fn export_sticker(s: &mut Session, p: &Value) -> Result<Value> {
    let check = b_p(p, "check").unwrap_or(false);
    let path = str_p(p, "path").map(str::to_string);
    let format = match (str_p(p, "format"), &path) {
        (Some(f), _) => f.to_ascii_lowercase(),
        (None, Some(path)) if path.to_ascii_lowercase().ends_with(".webm") => "webm".into(),
        _ => "tgs".into(),
    };
    let webm = match format.as_str() {
        "tgs" => false,
        "webm" => true,
        f => return Err(bad(ID, format!("unknown format `{f}` (tgs or webm)"))),
    };
    let path = match path {
        Some(p) => p,
        None if check => String::new(),
        None => return Err(bad(ID, "missing `path`")),
    };
    let cid = comp_id(s, p)?;
    let (emoji, force) = (b_p(p, "emoji").unwrap_or(false), b_p(p, "force").unwrap_or(false));
    let comp = s.project.comp(cid).ok_or(EngineError::NoComp)?.clone();
    let mut problems = precheck(&comp, webm, emoji);
    let (bytes, warnings) = if webm {
        if check || (!problems.is_empty() && !force) { (0, vec![]) } else { (export_webm(s, cid, &comp, &path, emoji, &mut problems)?, vec![]) }
    } else {
        let opts = ExportOptions { include_expressions: false, text_as_shapes: true, frame_rate: Some(FrameRate::new(TGS_FPS, 1)) };
        let services = s.services.clone();
        let read = move |f: &str| services.read_file(f).ok();
        let res = effectcraft_lottie::export_comp(&s.project, cid, &opts, &read).map_err(|e| EngineError::Other(e.to_string()))?;
        problems.extend(effectcraft_lottie::tgs_problems(&res));
        let data = effectcraft_lottie::to_tgs(&res);
        if data.len() > TGS_MAX_BYTES {
            problems.push(format!("file is {}; animated stickers may be at most {}", kb(data.len() as u64), kb(TGS_MAX_BYTES as u64)));
        }
        let warnings: Vec<String> = res.warnings.into_iter().filter(|w| !problems.iter().any(|p| p == w)).collect();
        if !check && (problems.is_empty() || force) {
            s.services.write_file(&path, &data).map_err(|e| EngineError::Other(format!("cannot write {path}: {e}")))?;
        }
        (data.len() as u64, warnings)
    };
    if !check && !problems.is_empty() && !force {
        return Err(refuse(&problems));
    }
    if !check {
        s.toast(format!("Exported Telegram sticker to {path} ({})", kb(bytes)));
    }
    Ok(json!({"path": path, "bytes": bytes, "format": format, "problems": problems, "warnings": warnings, "written": !check}))
}

/// Render the work area through the "Telegram Video Sticker" output module; re-encode at a lower
/// bitrate while the file is over 256 KB. Returns the size written.
fn export_webm(s: &Session, cid: ItemId, comp: &Comp, path: &str, emoji: bool, problems: &mut Vec<String>) -> Result<u64> {
    let exporter = s.exporter.clone().ok_or_else(|| EngineError::Other("video export is not available in this build".into()))?;
    let mut item = RenderQueueItem::new(0, cid);
    item.settings.time_span = TimeSpan::WorkArea;
    item.settings.frame_rate = Some(if comp.frame_rate.as_f64() > TELEGRAM_VIDEO_FPS as f64 { FrameRate::new(TELEGRAM_VIDEO_FPS, 1) } else { comp.frame_rate });
    item.settings.max_duration = Some(effectcraft_time::Tick::from_seconds_f64(TELEGRAM_MAX_SECONDS));
    item.output = telegram_video_sticker();
    if emoji {
        (item.output.resize.width, item.output.resize.height) = (EMOJI_SIZE, EMOJI_SIZE);
    }
    let (a, b) = item.settings.span(comp);
    item.output.bitrate_kbps = telegram_video_kbps((b - a).seconds());
    let mut bytes = 0;
    for _ in 0..WEBM_TRIES {
        let job = ExportJob {
            project: &s.project,
            footage: s.footage.as_ref(),
            expr: s.expr.as_deref(),
            accel: s.accel.as_deref(),
            item: &item,
            path,
            storage: s.storage_quota.as_deref(),
            label: "Telegram Sticker".into(),
            nested_switches: s.prefs.general.switches_affect_nested_comps,
        };
        bytes = exporter.export(&job, &mut |_, _| true).map_err(EngineError::Other)?.bytes;
        if bytes <= TELEGRAM_VIDEO_MAX_BYTES {
            return Ok(bytes);
        }
        // Scale the bitrate by how far over the file is, with 10 % more headroom.
        let k = TELEGRAM_VIDEO_MAX_BYTES as f64 / bytes as f64 * 0.9;
        item.output.bitrate_kbps = ((item.output.bitrate_kbps as f64 * k) as u32).max(50);
    }
    problems.push(format!(
        "file is {} after {WEBM_TRIES} encodes (written to {path}); video stickers may be at most {}",
        kb(bytes),
        kb(TELEGRAM_VIDEO_MAX_BYTES)
    ));
    Ok(bytes)
}

pub fn specs() -> Vec<CommandSpec> {
    vec![cmd!(
        "file.exportTelegramSticker",
        "Telegram Sticker...",
        ["File", "Export"],
        None,
        "{comp?, path (.tgs or .webm), format?: tgs|webm, emoji?: bool, force?: bool, check?: bool (report problems, write nothing)}",
        has_comp,
        export_sticker
    )]
}
