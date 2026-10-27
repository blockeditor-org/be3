use beui::native::Document;
use beui::native::build;
use beui::reactive::view;
use beui::{Rect, native::Context};

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

impl beui::native::App for FixtureApp {
    fn update(&mut self, context: &Context, rect: Rect) {
        self.document.show(context, rect);
    }
}
