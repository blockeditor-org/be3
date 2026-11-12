use beui::reactive::*;
pub use beui::styled::theme::NARROW_WIDTH;
use beui::styled::*;
use beui::{Align, Direction};
use beui::{NodeId, Vec2};

pub const SIDE_PANEL_WIDTH: f32 = 260.0;

const BAND_PADDING: Padding = Padding::symmetric(Space::Lg, Space::Md);
const BAND_SPACING: Space = Space::Md;

#[derive(Clone, Copy)]
struct ChromeLayout {
    narrow: Memo<bool>,
}

pub fn narrow_chrome() -> Memo<bool> {
    match use_context::<ChromeLayout>() {
        Some(layout) => layout.narrow,
        None => create_memo(|| false),
    }
}

#[component]
pub(crate) fn ChromeRoot(#[prop(children)] content: Render<()>) -> NodeId {
    view! {
        <Container>|_size: ReadSignal<Vec2>| {
            provide_context(ChromeLayout {
                narrow: narrower_than(NARROW_WIDTH),
            });
            content.call(())
        }</Container>
    }
}

#[component]
pub fn Toolbar(
    #[prop(default = true)] shown: Prop<bool>,
    #[prop(default = BAND_SPACING)] spacing: Prop<Space>,
    #[prop(default = false)] fit: Prop<bool>,
    #[prop(children)] children: Children<ListItem>,
) -> DynamicSegment<ListItem> {
    let row = NodeRef::new();
    arrow_focus(row, Direction::Horizontal);
    view! {
        <ToolbarFrame shown fit>
            <List
                @node_ref=&row
                direction=Direction::Horizontal
                align=Align::Center
                spacing={spacing}
                children={children}
            />
        </ToolbarFrame>
    }
}

#[component]
fn ToolbarFrame(shown: Prop<bool>, fit: Prop<bool>, children: Child) -> DynamicSegment<ListItem> {
    let shown = create_memo(move || shown.get());
    let fit = create_memo(move || fit.get());
    let squeezed = narrow_chrome();
    let narrow = create_memo(move || squeezed.get() && !fit.get());
    let roomy = create_memo(move || !narrow.get());
    let spread = create_memo(move || roomy.get().then_some(children));
    let scrolled = create_memo(move || narrow.get().then_some(children));
    view! {
        <Show keep_alive=true condition={shown}>
            <Frame fill=ColorRole::Surface>
                <List spacing=Space::None>
                    <Show condition={roomy}>
                        <Frame padding=BAND_PADDING>
                            <List
                                direction=Direction::Horizontal
                                align=Align::Center
                                spacing=Space::None
                            >
                                <ListItem grow=1.0>
                                    <Portal node={spread} />
                                </ListItem>
                            </List>
                        </Frame>
                    </Show>
                    <Show condition={narrow}>
                        <List
                            direction=Direction::Horizontal
                            align=Align::Center
                            spacing=Space::None
                        >
                            <ListItem grow=1.0>
                                <Scroll direction=Direction::Horizontal>
                                    <ListItem grow=1.0>
                                        <Frame padding=BAND_PADDING>
                                            <Portal node={scrolled} />
                                        </Frame>
                                    </ListItem>
                                </Scroll>
                            </ListItem>
                        </List>
                    </Show>
                    <Separator />
                </List>
            </Frame>
        </Show>
    }
}
