//! Typing on touch screens in the web build. iOS opens the on-screen keyboard only for a text
//! field focused by the user's own tap, and eframe focuses its hidden text input a frame after the
//! tap, from script: the keyboard stays down. So on touch screens the web shell lays eframe's
//! (transparent) text input over the text zones ([`end_pass`]), where a tap focuses it natively
//! and opens the keyboard; the shell then hands egui that tap ([`tap_text_zone`],
//! [`finger_events`]) and the field takes the typing. Drags and long presses that begin on the
//! input go to egui the same way.
//!
//! The zones are every text field ([`egui::TextEdit`]) and the values a tap turns into one (hot
//! text, number fields: [`text_zone`]) that no window, menu or popup covers.

use egui::{Context, Event, Id, LayerId, Pos2, Rect};

/// Publishes a value that a tap turns into a text field (hot text, a number field) as one of the
/// text zones ([`end_pass`]). (Text fields themselves are found without it.)
pub(crate) fn text_zone(resp: &egui::Response) {
    if !resp.enabled() {
        return;
    }
    let pass = resp.ctx.cumulative_pass_nr();
    resp.ctx.data_mut(|d| {
        let zones = d.get_temp_mut_or_default::<(u64, Vec<(Rect, LayerId)>)>(Id::new(ZONES));
        if zones.0 != pass {
            *zones = (pass, Vec::new());
        }
        zones.1.push((resp.interact_rect, resp.layer_id));
    });
}

/// The web shell's end of the pass: keeps the keyboard up while a tapped value turns into its text
/// field, then returns where a tap types, in points (see the module docs), to lay the text input
/// over. None while a field takes typing (or the keyboard is kept up for one): the input then
/// follows the text cursor. At the end of the pass, which knows every widget and whether a field
/// took typing.
pub fn end_pass(ctx: &Context) -> Vec<Rect> {
    let tapped: Option<(Rect, u64)> = ctx.data(|d| d.get_temp(Id::new(TAPPED)));
    // Reported between passes, after `pass` completed ones: hot text opens its field the pass
    // after the tap and focuses it the one after that. A text field takes the focus at once.
    if let Some((rect, pass)) = tapped {
        if ctx.cumulative_pass_nr() < pass.saturating_add(2) {
            ctx.output_mut(|o| {
                o.ime.get_or_insert(egui::output::IMEOutput {
                    purpose: egui::IMEPurpose::Normal,
                    rect,
                    cursor_rect: rect,
                    should_interrupt_composition: false,
                });
            });
        } else {
            ctx.data_mut(|d| d.remove::<(Rect, u64)>(Id::new(TAPPED)));
        }
    }
    let zones = if ctx.output(|o| o.ime.is_some()) { Vec::new() } else { shown_zones(ctx) };
    ctx.data_mut(|d| d.insert_temp(Id::new(SHOWN), zones.clone()));
    zones
}

/// The text input over the zones ([`end_pass`]) was tapped at `p` (points) and has the focus: the
/// shell hands egui the tap ([`finger_events`]), and the keyboard stays up until the field there
/// takes the typing. Returns the zone tapped, or none if no zone is there.
pub fn tap_text_zone(ctx: &Context, p: Pos2) -> Option<Rect> {
    let shown: Vec<Rect> = ctx.data(|d| d.get_temp(Id::new(SHOWN))).unwrap_or_default();
    let rect = shown.into_iter().find(|r| r.contains(p))?;
    let pass = ctx.cumulative_pass_nr();
    ctx.data_mut(|d| d.insert_temp(Id::new(TAPPED), (rect, pass)));
    ctx.request_repaint();
    Some(rect)
}

/// The events eframe sends for a finger on its canvas, at `pos` (points): a primary press as it
/// lands, moves, a release as it lifts. The web shell sends them itself for a touch on its text
/// input, which eframe's canvas listeners never see.
pub fn finger_events(phase: egui::TouchPhase, id: egui::TouchId, pos: Pos2) -> Vec<Event> {
    use egui::{PointerButton, TouchPhase};
    let button = |pressed| Event::PointerButton { pos, button: PointerButton::Primary, pressed, modifiers: egui::Modifiers::NONE };
    let touch = Event::Touch { device_id: egui::TouchDeviceId(0), id, phase, pos, force: None };
    match phase {
        TouchPhase::Start => vec![Event::PointerMoved(pos), button(true), touch],
        TouchPhase::Move => vec![Event::PointerMoved(pos), touch],
        TouchPhase::End | TouchPhase::Cancel => vec![button(false), Event::PointerGone, touch],
    }
}

/// A long press at `pos` (points), as the secondary click that opens the context menu, for a long
/// press on the web shell's text input.
pub fn long_press_events(pos: Pos2) -> Vec<Event> {
    let button = |pressed| Event::PointerButton { pos, button: egui::PointerButton::Secondary, pressed, modifiers: egui::Modifiers::NONE };
    vec![Event::PointerMoved(pos), button(true), button(false)]
}

const ZONES: &str = "effectcraft.touch.zones";
const SHOWN: &str = "effectcraft.touch.shown";
const TAPPED: &str = "effectcraft.touch.tapped";

/// This pass's zones that no window, menu or popup covers, even in part: their layer takes a tap
/// at their centre (so no modal dialog is over them) and no interactable layer above it overlaps
/// them.
fn shown_zones(ctx: &Context) -> Vec<Rect> {
    let pass = ctx.cumulative_pass_nr();
    let mut zones: Vec<(Rect, LayerId)> =
        ctx.data(|d| d.get_temp::<(u64, Vec<(Rect, LayerId)>)>(Id::new(ZONES))).filter(|z| z.0 == pass).map(|z| z.1).unwrap_or_default();
    // The text fields: the clickable widgets with a text field's state. (A tap focuses them.)
    let clickable: Vec<(Id, Rect, LayerId)> = ctx.viewport(|v| {
        v.this_pass
            .widgets
            .layers()
            .flat_map(|(_, w)| w.iter())
            .filter(|w| w.enabled && w.sense.senses_click())
            .map(|w| (w.id, w.interact_rect, w.layer_id))
            .collect()
    });
    zones.extend(clickable.into_iter().filter(|(id, ..)| egui::TextEdit::load_state(ctx, *id).is_some()).map(|(_, rect, layer)| (rect, layer)));
    // Back to front.
    let layers: Vec<LayerId> = ctx.memory(|m| m.layer_ids().filter(|l| m.areas().is_visible(l)).collect());
    let covered = |r: Rect, layer: LayerId| {
        let at = layers.iter().position(|l| *l == layer);
        layers
            .iter()
            .enumerate()
            .filter(|(i, l)| at.map_or(l.order > layer.order, |at| *i > at))
            .filter_map(|(_, l)| egui::AreaState::load(ctx, l.id))
            .any(|a| a.interactable && a.rect().intersects(r))
    };
    zones
        .into_iter()
        .filter(|(rect, layer)| {
            let r = rect.shrink(1.0);
            r.is_positive() && ctx.layer_id_at(r.center()) == Some(*layer) && !covered(r, *layer)
        })
        .map(|(rect, _)| rect)
        .collect()
}
