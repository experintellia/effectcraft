//! File ▸ Export ▸ Telegram Sticker…: pick the format, see Telegram's checks for the active
//! composition (`file.exportTelegramSticker` with `check`), then choose where to save.

use egui::{Color32, vec2};
use serde_json::{Value, json};

use crate::i18n::tr;
use crate::theme::Tokens;
use crate::{Dialog, EffectcraftApp};

const ID: &str = "file.exportTelegramSticker";

#[derive(Clone, Debug, Default)]
pub struct StickerDraft {
    pub webm: bool,
    pub emoji: bool,
    /// The last check's result (`problems`, `warnings`), or its error.
    pub report: Option<Result<Value, String>>,
}

impl StickerDraft {
    fn params(&self) -> Value {
        json!({"format": if self.webm { "webm" } else { "tgs" }, "emoji": self.emoji})
    }
}

fn check(app: &mut EffectcraftApp) {
    let mut p = app.dialog_state.sticker.params();
    p["check"] = json!(true);
    app.dialog_state.sticker.report = Some(app.session.execute(ID, p).map_err(|e| e.to_string()));
}

/// Open the dialog on the active composition.
pub fn open(app: &mut EffectcraftApp) -> Result<(), String> {
    app.session.active_comp().ok_or("open a composition first")?;
    app.dialog_state.sticker.report = None;
    check(app);
    app.dialog = Some(Dialog::TelegramSticker);
    Ok(())
}

fn lines(v: &Value, key: &str) -> Vec<String> {
    v.get(key).and_then(Value::as_array).map(|a| a.iter().filter_map(|x| x.as_str().map(str::to_string)).collect()).unwrap_or_default()
}

pub fn show(app: &mut EffectcraftApp, ctx: &egui::Context, t: &Tokens) {
    let mut d = app.dialog_state.sticker.clone();
    let (mut export, mut cancel, mut changed) = (false, false, false);
    let (problems, warnings, error) = match &d.report {
        Some(Ok(v)) => (lines(v, "problems"), lines(v, "warnings"), None),
        Some(Err(e)) => (vec![], vec![], Some(e.clone())),
        None => (vec![], vec![], None),
    };
    super::dialogs::modal(ctx, tr("Telegram Sticker"), vec2(460.0, 360.0), t, |ui| {
        ui.horizontal(|ui| {
            ui.label(tr("Format:"));
            for (webm, label, id) in [(false, "Animated (.tgs)", "tgs"), (true, "Video (.webm)", "webm")] {
                let r = ui.radio(d.webm == webm, tr(label));
                app.auto.add(&format!("dialog.sticker.{id}"), r.rect, label);
                if r.clicked() && d.webm != webm {
                    d.webm = webm;
                    changed = true;
                }
            }
        });
        let r = ui.checkbox(&mut d.emoji, tr("Custom emoji (100×100)"));
        app.auto.add("dialog.sticker.emoji", r.rect, "Custom emoji");
        changed |= r.changed();
        ui.add_space(8.0);
        ui.label(egui::RichText::new(tr("Telegram checks")).font(Tokens::medium(12.5)));
        egui::ScrollArea::vertical().max_height(190.0).min_scrolled_height(190.0).auto_shrink([false, false]).show(ui, |ui| {
            if let Some(e) = &error {
                ui.colored_label(t.danger, e);
            } else if problems.is_empty() {
                ui.label(tr("Ready: no Telegram limits are broken."));
            }
            for p in &problems {
                ui.colored_label(t.danger, format!("✕ {p}"));
            }
            for w in &warnings {
                ui.label(egui::RichText::new(format!("• {w}")).color(t.text_dim));
            }
        });
        ui.add_space(10.0);
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let label = if problems.is_empty() { tr("Export...") } else { tr("Export Anyway...") };
            let r = ui.add_enabled(error.is_none(), egui::Button::new(egui::RichText::new(label).color(Color32::WHITE)).fill(t.accent));
            app.auto.add("dialog.sticker.export", r.rect, "Export");
            export = r.clicked();
            let r = ui.button(tr("Cancel"));
            app.auto.add("dialog.sticker.cancel", r.rect, "Cancel");
            cancel = r.clicked();
        });
    });
    app.dialog_state.sticker = d.clone();
    if changed {
        check(app);
    }
    if export {
        app.dialog = None;
        let mut p = d.params();
        p["force"] = json!(!problems.is_empty());
        // No `path` yet: the menu path asks for one with the save dialog.
        if let Err(e) = crate::menus::invoke(app, ctx, ID, p) {
            app.ui.status = e;
        }
    } else if cancel {
        app.dialog = None;
    }
}
