use super::*;
use crate::native::Document;
use crate::primitives::*;

struct Hosting(Document);

impl Program for Hosting {
    fn update(&mut self, context: &Context, rect: Rect) {
        self.0.show(context, rect);
    }

    fn holdable(&self) -> Vec<String> {
        vec!["be3.text".to_owned()]
    }
}

#[test]
fn a_driver_holds_only_an_app_the_program_runs() {
    let document = build_node(|| {
        view! {
            <Text>"Held"</Text>
        }
    });
    let mut driven = stepped(Hosting(document));
    let refused = driven
        .ask(&["hold", "be3.txt"])
        .expect_err("a name the program does not run is refused");
    assert!(
        refused.contains("not a running app's id") && refused.contains("be3.text"),
        "the error names the ids it could be: {refused}"
    );
    driven
        .ask(&["hold", "be3.text"])
        .expect("a running app is held");
    driven.ask(&["hold", "off"]).expect("and let go");
}
