use beui::reactive::{Frame, List, build, create_signal, view};
use beui::styled::{Body, TextInput, use_theme};
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
            let (state, set_state) = create_signal("Hello, world!".to_owned());
            let theme = use_theme();
            view! {
                <Frame
                    color={theme.background.clone()}
                    radius=0
                    padding_horizontal=16.0
                    padding_vertical=16.0
                >
                    <List spacing=8.0>
                        <Body content={state.clone()} />
                        <TextInput
                            value={state}
                            label="Text"
                            on_change={move |value| set_state.set(value)}
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
