//! Typing on a touch screen (the web build on phones, iOS above all): the web shell lays its text
//! input over the text zones, the text fields and the values a tap turns into one, so that a tap
//! focuses it natively and opens the on-screen keyboard. It then hands egui the tap, and the
//! keyboard stays up until the field takes the typing.

use effectcraft_engine::Session;
use effectcraft_ui_egui::{Dialog, EffectcraftApp, touch};
use egui::{Event, Key, Modifiers, Pos2, Rect, TouchId, TouchPhase, pos2, vec2};
use egui_kittest::Harness;
use serde_json::json;

/// A comp with a solid whose Transform is revealed in the Timeline; its Opacity's uid.
fn harness() -> (Harness<'static, EffectcraftApp>, u64) {
    let mut s = Session::default();
    s.execute("comp.new", json!({"name": "Main", "width": 320, "height": 180, "duration": 4, "fps": 24})).unwrap();
    let layer = s.execute("layer.newSolid", json!({"name": "Plate", "color": "#406080"})).unwrap()["layer"].as_u64().unwrap();
    let tr = s.active_comp().unwrap().layer(effectcraft_engine::project::LayerId(layer)).unwrap().props.sub("transform").unwrap().clone();
    let opacity = tr.get("opacity").unwrap().uid;
    let mut h = Harness::builder().with_size(vec2(1600.0, 1000.0)).build_eframe(|_| EffectcraftApp::new(s));
    h.state_mut().ui.timeline.open_layers.insert(layer);
    h.state_mut().ui.timeline.open_groups.insert(tr.uid);
    // What the web shell does at the end of every pass; the zones it would cover.
    h.ctx.on_end_pass(
        "test.text_zones",
        std::sync::Arc::new(|ui: &mut egui::Ui| {
            let zones = touch::end_pass(ui.ctx());
            ui.ctx().data_mut(|d| d.insert_temp(egui::Id::new("test.text_zones"), zones));
        }),
    );
    h.run_steps(4);
    (h, opacity)
}

fn center(h: &Harness<'_, EffectcraftApp>, id: &str) -> Pos2 {
    let e = h.state().auto.find(id).unwrap_or_else(|| panic!("no {id}"));
    pos2(e.rect[0] + e.rect[2] / 2.0, e.rect[1] + e.rect[3] / 2.0)
}

fn zones(h: &Harness<'_, EffectcraftApp>) -> Vec<Rect> {
    h.ctx.data(|d| d.get_temp(egui::Id::new("test.text_zones"))).unwrap_or_default()
}

fn ime(h: &Harness<'_, EffectcraftApp>) -> bool {
    h.output().platform_output.ime.is_some()
}

fn opacity(h: &Harness<'_, EffectcraftApp>) -> f64 {
    let comp = h.state().session.active_comp().unwrap();
    let layer = comp.layers.first().unwrap();
    layer.props.sub("transform").unwrap().get("opacity").unwrap().value.components()[0]
}

/// What the web shell does when iOS has focused its text input after a tap at `p`: report it,
/// then hand egui the tap.
fn tap_input(h: &mut Harness<'_, EffectcraftApp>, p: Pos2) {
    assert!(touch::tap_text_zone(&h.ctx, p).is_some_and(|z| z.contains(p)), "the input lies at {p:?}");
    let mut events = touch::finger_events(TouchPhase::Start, TouchId(1), p);
    events.extend(touch::finger_events(TouchPhase::End, TouchId(1), p));
    h.input_mut().events.extend(events);
}

fn key(h: &mut Harness<'_, EffectcraftApp>, key: Key) {
    h.input_mut().events.push(Event::Key { key, physical_key: None, pressed: true, repeat: false, modifiers: Modifiers::NONE });
    h.input_mut().events.push(Event::Key { key, physical_key: None, pressed: false, repeat: false, modifiers: Modifiers::NONE });
    h.run_steps(2);
}

#[test]
fn a_tap_on_the_input_over_a_hot_value_types_into_it_with_the_keyboard_up() {
    let (mut h, uid) = harness();
    let p = center(&h, &format!("timeline.prop.{uid}.value"));
    assert!(!ime(&h));
    tap_input(&mut h, p);
    // The value turns into its text field over two passes: the keyboard must not drop meanwhile.
    for _ in 0..4 {
        h.step();
        assert!(ime(&h), "the keyboard stays up");
    }
    assert!(h.ctx.egui_wants_keyboard_input(), "the value's field has the keyboard");
    assert!(zones(&h).is_empty(), "while typing the input follows the text cursor");
    h.input_mut().events.push(Event::Text("42".into()));
    h.step();
    key(&mut h, Key::Enter);
    assert_eq!(opacity(&h), 42.0);
    assert!(!ime(&h), "Enter lets the keyboard go");
}

#[test]
fn a_tap_on_the_input_over_a_search_field_focuses_it() {
    let (mut h, _) = harness();
    let p = center(&h, "project.search");
    tap_input(&mut h, p);
    h.step();
    assert!(ime(&h), "the field takes the typing at once");
    assert!(h.ctx.egui_wants_keyboard_input());
    h.input_mut().events.push(Event::Text("Pl".into()));
    h.step();
    assert_eq!(h.state().ui.project_search, "Pl");
}

#[test]
fn a_tap_that_starts_no_typing_lets_the_kept_keyboard_go() {
    let (mut h, _) = harness();
    let p = center(&h, "project.search");
    // Reported, but egui never gets the tap (say, a field that went away).
    assert!(touch::tap_text_zone(&h.ctx, p).is_some());
    h.step();
    assert!(ime(&h));
    h.step();
    assert!(ime(&h));
    h.step();
    assert!(!ime(&h), "the keyboard goes once no field took the typing");
    assert!(zones(&h).iter().any(|z| z.contains(p)), "and the input lies over the fields again");
}

#[test]
fn no_text_input_between_the_fields() {
    let (h, _) = harness();
    let zones = zones(&h);
    assert!(!zones.is_empty());
    let search = zones.iter().copied().find(|z| z.contains(center(&h, "project.search"))).expect("the search field takes a tap");
    let below = search.center_bottom() + vec2(0.0, 3.0);
    assert!(!zones.iter().any(|z| z.contains(below)), "the panel under it keeps its taps");
    assert!(touch::tap_text_zone(&h.ctx, below).is_none());
}

#[test]
fn a_drag_from_the_input_over_a_hot_value_scrubs_it() {
    let (mut h, uid) = harness();
    let p = center(&h, &format!("timeline.prop.{uid}.value"));
    let before = opacity(&h);
    // What the web shell sends for a drag that began on its text input.
    for (phase, pos) in
        [(TouchPhase::Start, p), (TouchPhase::Move, p + vec2(-12.0, 0.0)), (TouchPhase::Move, p + vec2(-40.0, 0.0)), (TouchPhase::End, p + vec2(-40.0, 0.0))]
    {
        h.input_mut().events.extend(touch::finger_events(phase, TouchId(2), pos));
        h.step();
    }
    h.run_steps(2);
    assert!(opacity(&h) < before, "the drag scrubs the value down");
    assert!(!ime(&h), "a drag doesn't type");
}

#[test]
fn no_text_input_over_fields_a_dialog_covers() {
    let (mut h, _) = harness();
    let search = center(&h, "project.search");
    h.state_mut().dialog = Some(Dialog::NewComp);
    h.run_steps(2);
    assert!(ime(&h) && zones(&h).is_empty(), "the dialog puts the keyboard on its name field: no input over the fields");
    h.ctx.memory_mut(|m| {
        if let Some(id) = m.focused() {
            m.surrender_focus(id);
        }
    });
    h.run_steps(2);
    let zones = zones(&h);
    assert!(!zones.is_empty(), "the dialog's fields take a tap");
    assert!(!zones.iter().any(|z| z.contains(search)), "the search field under the dialog doesn't: {zones:?}");
}

#[test]
fn no_text_input_over_a_field_a_popup_covers_in_part() {
    let (mut h, _) = harness();
    let p = center(&h, "project.search");
    let field = zones(&h).into_iter().find(|z| z.contains(p)).expect("the search field takes a tap");
    // A popup over the left of the field, clear of its centre and corners.
    let strip = Rect::from_min_size(field.left_center() + vec2(4.0, -2.0), vec2(12.0, 4.0));
    h.ctx.on_end_pass(
        "test.popup",
        std::sync::Arc::new(move |ui: &mut egui::Ui| {
            egui::Area::new(egui::Id::new("test.popup")).order(egui::Order::Foreground).fixed_pos(strip.min).show(ui.ctx(), |ui| {
                ui.allocate_exact_size(strip.size(), egui::Sense::click());
            });
        }),
    );
    h.run_steps(3);
    assert!(!zones(&h).iter().any(|z| z.contains(p)), "a tap on the popup reaches it");
}
