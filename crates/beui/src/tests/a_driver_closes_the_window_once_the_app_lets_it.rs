use super::*;
use crate::primitives::*;
use crate::reactive::*;

struct Guarded {
    document: Document,
    unsaved: ReadSignal<bool>,
    asking: Signal<bool>,
}

impl App for Guarded {
    fn update(&mut self, context: &Context, rect: Rect) {
        self.document.show(context, rect);
    }

    fn close_requested(&mut self) -> bool {
        if self.unsaved.get_untracked() {
            let asking = self.asking;
            with_installed(&mut self.document, |_| asking.set(true));
        }
        !self.unsaved.get_untracked()
    }
}

#[test]
fn a_driver_closes_the_window_once_the_app_lets_it() {
    let _root = crate::reactive::RootScope::new();
    let asked = create_signal(false);
    let document = build_node(move || {
        view! {
            <List spacing=0.0>
                <Text>"Editing"</Text>
                <Show condition={asked}>
                    <Text>"Save changes?"</Text>
                </Show>
            </List>
        }
    });
    let unsaved = create_signal(true);
    let mut driven = headless(Guarded {
        document,
        unsaved: unsaved.read_only(),
        asking: asked,
    });

    let changes = driven
        .ask(&["close"])
        .expect("a window that stays open answers");
    assert!(
        changes.contains("+") && changes.contains("Save changes?"),
        "an app that stays open answers with what it showed instead:\n{changes}"
    );
    assert!(!driven.closed(), "the app kept its window open");

    unsaved.set(false);
    assert_eq!(
        driven.ask(&["close"]),
        Ok("The window closed.\n".to_owned())
    );
    assert!(
        driven.closed(),
        "the frame tells the platform to close the window"
    );
}
