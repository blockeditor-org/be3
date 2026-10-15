use super::*;
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
    let document = build(|| {
        view! {
            <Text string="Desktop" />
        }
    });
    let mut driven = Driven::with_app(Hosting(document), false);
    assert_eq!(
        driven
            .ask(&["env"])
            .expect("the driver reads the environment"),
        "XDG_RUNTIME_DIR=/tmp/be-wayland-1\nWAYLAND_DISPLAY=wayland-0\n",
        "a program started with these variables shows its windows inside the app"
    );

    let plain = build(|| {
        view! {
            <Text string="Plain" />
        }
    });
    let mut driven = Driven::new(plain);
    assert_eq!(
        driven
            .ask(&["env"])
            .expect("the driver reads the environment"),
        "",
        "an app that hosts no programs gives them nothing"
    );
}
