use beui::reactive::*;
use beui::styled::*;
use beui::{Rect, native::App, native::Context};
use beui_host::native::Document;
use beui_host::native::build;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    beui_host::native::run(SurveyApp::new())
}

struct SurveyApp {
    document: Document,
}

impl SurveyApp {
    fn new() -> Self {
        let document = build(|| {
            let state = create_signal("Hello, world!".to_owned());
            view! {
                <Root title="beui survey">
                    <Frame padding=Space::Xl>
                        <List spacing=Space::Md>
                            <Text>{state}</Text>
                            <TextInput value={state} label="Text" />
                        </List>
                    </Frame>
                </Root>
            }
        });
        Self { document }
    }
}

impl App for SurveyApp {
    fn update(&mut self, context: &Context, rect: Rect) {
        self.document.show(context, rect);
    }
}
