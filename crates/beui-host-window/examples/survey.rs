use beui::reactive::*;
use beui::styled::*;
use beui::{Rect, native::Context, native::Program};
use beui_host::native::Document;
use beui_host::native::build;

beui_host_window::main! {
    async fn main() -> Result<(), Box<dyn std::error::Error>> {
        beui_host_window::run_program(beui_host_window::RunOptions::default(), SurveyProgram::new())
            .await
    }
}

struct SurveyProgram {
    document: Document,
}

impl SurveyProgram {
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

impl Program for SurveyProgram {
    fn update(&mut self, context: &Context, rect: Rect) {
        self.document.show(context, rect);
    }
}
