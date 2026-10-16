//! The webxdc app runs at a phone's width: the Home screen's pages keep their controls on screen,
//! the quick-start tiles apart and each tutorial's Start button inside its card.

use effectcraft_engine::Session;
use effectcraft_ui_egui::EffectcraftApp;
use egui_kittest::Harness;

const W: f32 = 390.0;

fn rect(e: &[f32; 4]) -> egui::Rect {
    egui::Rect::from_min_size(egui::pos2(e[0], e[1]), egui::vec2(e[2], e[3]))
}

#[test]
fn home_pages_fit_a_phone() {
    let mut app = EffectcraftApp::new(Session::default());
    app.ui.start_screen = true;
    let mut h = Harness::builder().with_size(egui::vec2(W, 844.0)).build_eframe(move |_| app);
    for (templates, learn) in [(false, false), (true, false), (false, true)] {
        h.state_mut().ui.home_templates = templates;
        h.state_mut().ui.home_learn = learn;
        h.run_steps(3);
        let auto = &h.state().auto;
        let page = format!("templates {templates}, learn {learn}");
        for e in auto.query("home.") {
            assert!(e.rect[0] >= 0.0 && e.rect[0] + e.rect[2] <= W + 0.5, "{page}: {} at {:?} leaves the screen", e.id, e.rect);
        }
        let tiles: Vec<_> = ["home.app.newComp", "home.file.openDemoProject", "home.file.import", "home.templates"]
            .iter()
            .filter_map(|id| auto.query(id).into_iter().find(|e| e.id == *id).map(|e| rect(&e.rect)))
            .collect();
        for (i, a) in tiles.iter().enumerate() {
            for b in tiles.iter().skip(i + 1) {
                assert!(!a.intersects(*b), "{page}: quick-start tiles overlap: {a:?} {b:?}");
            }
        }
        for start in auto.query("home.learn.").into_iter().filter(|e| e.id.ends_with(".start")) {
            let card = auto.query(start.id.trim_end_matches(".start")).into_iter().find(|e| !e.id.ends_with(".start")).map(|e| rect(&e.rect));
            assert!(card.is_some_and(|c| c.contains_rect(rect(&start.rect))), "{page}: {} outside its card", start.id);
        }
        if learn {
            assert!(!auto.query("home.learn.").is_empty(), "the tutorials show");
        }
    }
}
