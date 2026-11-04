use beui::native::Document;
use beui::reactive::view;
use beui::{Rect, native::Context};
use beui_host::native::build;

use crate::Fixture;

pub struct FixtureProgram {
    document: Document,
}

impl FixtureProgram {
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

impl beui::native::Program for FixtureProgram {
    fn update(&mut self, context: &Context, rect: Rect) {
        self.document.show(context, rect);
    }
}
