use super::*;

#[test]
fn a_counter_app_counts_its_clicks() {
    let app = example_counter_app::CounterApp::new();
    let mut test = DocumentTest::new(app.document, Vec2::new(320.0, 160.0));

    test.click("counter.add");
    test.click("counter.add");

    test.wait(beui::testing::Target::text("Clicked 2 times"));
    test.snapshot("a_counter_app_counts_its_clicks");
}

mod example_counter_app {
    use beui::reactive::*;
    use beui::styled::*;
    use beui::{Context, Document, NodeId, Rect, Sides};
    use beui_macros::component;

    pub struct CounterApp {
        pub document: Document,
    }

    impl CounterApp {
        pub fn new() -> Self {
            let document = build(|| {
                view! {
                    <Root title={"Counter".to_owned()}>
                        <Counter />
                    </Root>
                }
            });
            Self { document }
        }
    }

    impl beui::App for CounterApp {
        fn update(&mut self, context: &Context, rect: Rect) {
            self.document.show(context, rect);
        }
    }

    #[component]
    fn Counter() -> NodeId {
        let count = create_signal(0);
        let label = create_memo(move || format!("Clicked {} times", count.get()));
        view! {
            <Frame padding={Sides::all(16.0)}>
                <List spacing=8.0>
                    <Text>{label}</Text>
                    <Button
                        @test_id="counter.add"
                        label="Add one"
                        variant=ButtonVariant::Primary
                        on_click={move || count.update(|count| *count += 1)}
                    />
                </List>
            </Frame>
        }
    }
}
