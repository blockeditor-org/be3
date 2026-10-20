use beui::reactive::*;
use beui::styled::*;
use beui::{App, Color32, Context, Document, Rect};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    beui::run("beui survey", SurveyApp::new())
}

struct SurveyApp {
    document: Document,
}

impl SurveyApp {
    fn new() -> Self {
        let document = build(|| {
            let state = create_signal("Hello, world!".to_owned());
            view! {
                <Frame fill=ColorRole::Background padding=16.0>
                    <List spacing=8.0>
                        <Text string={state} />
                        <TextInput
                            value={state}
                            label="Text"
                        />
                    </List>
                </Frame>
            }
        });
        Self { document }
    }
}

impl App for SurveyApp {
    fn update(&mut self, context: &Context, rect: Rect) {
        self.document.show(context, rect);
    }

    fn clear_color(&self) -> Color32 {
        self.document.theme().background
    }
}
