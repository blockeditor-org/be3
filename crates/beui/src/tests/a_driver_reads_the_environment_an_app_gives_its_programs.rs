use super::*;
use crate::native::Document;
use crate::primitives::*;
use crate::reactive::*;

struct Hosting(Document);

impl App for Hosting {
    fn update(&mut self, context: &Context, rect: Rect) {
        self.0.show(context, rect);
    }

    fn environment(&self) -> Vec<(String, String)> {
        vec![
            ("XDG_RUNTIME_DIR".to_owned(), "/tmp/be-wayland-1".to_owned()),
            ("WAYLAND_DISPLAY".to_owned(), "wayland-0".to_owned()),
        ]
    }
}

#[test]
fn a_driver_reads_the_environment_an_app_gives_its_programs() {
    let document = build_node(|| {
        view! {
            <Text>"Desktop"</Text>
        }
    });
    let mut driven = stepped(Hosting(document));
    assert_eq!(
        driven
            .ask(&["env"])
            .expect("the driver reads the environment"),
        "XDG_RUNTIME_DIR=/tmp/be-wayland-1\nWAYLAND_DISPLAY=wayland-0\n",
        "a program started with these variables shows its windows inside the app"
    );

    let plain = build_node(|| {
        view! {
            <Text>"Plain"</Text>
        }
    });
    let mut driven = stepped(crate::testing::Showing(plain));
    assert_eq!(
        driven
            .ask(&["env"])
            .expect("the driver reads the environment"),
        "",
        "an app that hosts no programs gives them nothing"
    );
}
