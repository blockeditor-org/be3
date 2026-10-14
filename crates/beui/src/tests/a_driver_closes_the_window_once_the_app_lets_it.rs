use super::*;
use crate::reactive::{List, Show, Text, build, view};

struct Guarded {
    document: Document,
    unsaved: ReadSignal<bool>,
    asking: WriteSignal<bool>,
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
    let (asked, asking) = create_signal(false);
    let document = build(move || {
        view! {
            <List spacing=0.0>
                <Text string="Editing" />
                <Show condition={asked}>
                    <Text string="Save changes?" />
                </Show>
            </List>
        }
    });
    let (unsaved, set_unsaved) = create_signal(true);
    let mut driven = Driven::with_app(
        Guarded {
            document,
            unsaved,
            asking,
        },
        true,
    );

    let changes = driven
        .ask(&["close"])
        .expect("a window that stays open answers");
    assert!(
        changes.contains("+") && changes.contains("Save changes?"),
        "an app that stays open answers with what it showed instead:\n{changes}"
    );
    assert!(!driven.closed, "the app kept its window open");

    set_unsaved.set(false);
    assert_eq!(
        driven.ask(&["close"]),
        Ok("The window closed.\n".to_owned())
    );
    assert!(
        driven.closed,
        "the frame tells the adapter to close the window"
    );
}
