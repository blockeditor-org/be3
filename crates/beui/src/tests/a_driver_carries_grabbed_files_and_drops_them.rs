use super::*;

struct Recording(Signal<Vec<String>>);

impl App for Recording {
    fn update(&mut self, context: &Context, _rect: Rect) {
        context.input(|input| {
            for event in &input.events {
                let heard = match event {
                    Event::FileHovered => "hovered".to_owned(),
                    Event::FileHoverCancelled => "cancelled".to_owned(),
                    Event::FileDropped(file) => format!("dropped {}", file.name),
                    Event::PointerMoved(pos) => format!("moved {},{}", pos.x, pos.y),
                    _ => continue,
                };
                self.0.update(|log| log.push(heard));
            }
        });
    }
}

#[test]
fn a_driver_carries_grabbed_files_and_drops_them() {
    let _root = crate::reactive::RootScope::new();
    let heard = create_signal(Vec::new());
    let mut driven = headless(Recording(heard));

    driven.ask(&["move", "10,10"]).expect("the pointer moves");
    driven
        .ask(&["grab", "/notes/a.txt", "/notes/b.txt"])
        .expect("the files are grabbed");
    driven.ask(&["move", "50,60"]).expect("they are carried");
    driven.ask(&["drop", "80,90"]).expect("they are dropped");
    assert!(driven.ask(&["drop"]).is_err(), "nothing is left to drop");
    driven
        .ask(&["grab", "/notes/c.txt"])
        .expect("another is grabbed");
    driven.ask(&["ungrab"]).expect("and taken away");
    assert_eq!(
        heard.get_untracked(),
        [
            "moved 10,10",
            "moved 10,10",
            "hovered",
            "moved 50,60",
            "moved 80,90",
            "dropped a.txt",
            "dropped b.txt",
            "moved 80,90",
            "hovered",
            "cancelled",
        ]
    );
}
