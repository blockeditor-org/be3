use beui::native::Document;
use beui::reactive::{build, view};
use beui::{Context, Rect};

use crate::Fixture;

pub struct FixtureApp {
    document: Document,
}

impl FixtureApp {
    pub fn new(name: String) -> Self {
        Self {
            document: build(move || {
                view! {
                    <Fixture name={name} />
                }
            }),
        }
    }
}

impl beui::App for FixtureApp {
    fn update(&mut self, context: &Context, rect: Rect) {
        self.document.show(context, rect);
    }
}
