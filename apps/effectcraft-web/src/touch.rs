//! Typing on touch screens: one tap on a text field (or a value a tap types into) opens the
//! on-screen keyboard, also on iOS. The page lays eframe's text input over those fields
//! (`touchCover` in `js/host.js`) so that the tap focuses it natively, then hands egui the tap
//! and the drags and long presses that began on it ([`effectcraft_ui_egui::touch`]).

use std::cell::Cell;
use std::rc::Rc;

use effectcraft_ui_egui::touch;
use egui::{Event, Pos2, TouchId, TouchPhase};
use wasm_bindgen::prelude::*;

#[wasm_bindgen(module = "/js/host.js")]
extern "C" {
    #[wasm_bindgen(js_name = touchInstall)]
    fn touch_install(canvas: &web_sys::HtmlCanvasElement, hold_ms: f64, on_tap: &JsValue, on_touch: &JsValue) -> bool;
    #[wasm_bindgen(js_name = touchCover)]
    fn touch_cover(rects: &[f32]);
}

/// Events for egui that eframe's canvas listeners never saw (touches on its text input).
pub type Touches = Rc<Cell<Vec<Event>>>;

/// Listen on a touch screen; the events to hand egui each frame (`raw_input_hook`).
pub fn install(ctx: &egui::Context, canvas: &web_sys::HtmlCanvasElement) -> Touches {
    let events: Touches = Rc::default();
    let send = {
        let (events, ctx) = (events.clone(), ctx.clone());
        move |more: Vec<Event>| {
            let mut all = events.take();
            all.extend(more);
            events.set(all);
            ctx.request_repaint();
        }
    };
    let on_tap = Closure::<dyn FnMut(f32, f32)>::new({
        let (ctx, send) = (ctx.clone(), send.clone());
        move |x: f32, y: f32| {
            let p = point(&ctx, x, y);
            let Some(zone) = touch::tap_text_zone(&ctx, p) else { return };
            // Focused now: the input can shrink to the zone tapped (iOS keeps a focused input in view).
            touch_cover(&css_rects(&ctx, &[zone]));
            let mut tap = touch::finger_events(TouchPhase::Start, TouchId(0), p);
            tap.extend(touch::finger_events(TouchPhase::End, TouchId(0), p));
            send(tap);
        }
    });
    let on_touch = Closure::<dyn FnMut(String, i32, f32, f32)>::new({
        let ctx = ctx.clone();
        move |kind: String, id: i32, x: f32, y: f32| {
            let (p, id) = (point(&ctx, x, y), TouchId::from(id));
            send(match kind.as_str() {
                "start" => touch::finger_events(TouchPhase::Start, id, p),
                "move" => touch::finger_events(TouchPhase::Move, id, p),
                "end" => touch::finger_events(TouchPhase::End, id, p),
                "cancel" => touch::finger_events(TouchPhase::Cancel, id, p),
                "long" => touch::long_press_events(p),
                _ => vec![Event::PointerGone],
            });
        }
    });
    // As long as egui waits for a long press.
    let hold_ms = ctx.options(|o| o.input_options.max_click_duration * 1000.0).clamp(0.0, 10_000.0);
    if touch_install(canvas, hold_ms, on_tap.as_ref(), on_touch.as_ref()) {
        ctx.on_end_pass("effectcraft.touch", std::sync::Arc::new(|ui: &mut egui::Ui| touch_cover(&css_rects(ui.ctx(), &touch::end_pass(ui.ctx())))));
    }
    // Listening for the page's lifetime.
    on_tap.forget();
    on_touch.forget();
    events
}

/// CSS px from the canvas's top left → points.
fn point(ctx: &egui::Context, x: f32, y: f32) -> Pos2 {
    let zoom = ctx.zoom_factor();
    egui::pos2(x / zoom, y / zoom)
}

/// Rects in points → [x, y, width, height, …] in CSS px.
fn css_rects(ctx: &egui::Context, rects: &[egui::Rect]) -> Vec<f32> {
    let zoom = ctx.zoom_factor();
    rects.iter().flat_map(|r| [r.min.x * zoom, r.min.y * zoom, r.width() * zoom, r.height() * zoom]).collect()
}
