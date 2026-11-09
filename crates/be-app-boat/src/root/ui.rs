use be_sdk_beui::Editor;
use be_sdk_beui::beui::icons::ICON_GLOBE;
use be_sdk_beui::beui::reactive::*;
use be_sdk_beui::beui::styled::*;
use be_sdk_beui::beui::{Align, Direction, NodeId};

use super::list::Sandboxes;
use super::sign_in::SignIn;
use super::state::State;

pub(super) const PADDING: Space = Space::Xl2;
pub(super) const SECTION_SPACING: Space = Space::Xl;
pub(super) const SPACING: Space = Space::Sm;

/// The block's page: why it can't run on the web, the sign-in page while the viewer has no
/// key boat takes for the block's account, and otherwise the sandboxes.
#[component]
pub fn Boat(editor: Editor) -> NodeId {
    let state = State::new(editor);
    let web = state.web;
    let signing_in = create_memo(move || {
        !web.get() && (state.key.with(Option::is_none) || state.reconnecting.get())
    });
    let listing = create_memo(move || !web.get() && !signing_in.get());
    view! {
        <Scroll>
            <Frame padding=PADDING>
                <List spacing=SECTION_SPACING>
                    <Show condition={web}>
                        <OnTheWeb />
                    </Show>
                    <Show condition={signing_in}>
                        <SignIn state />
                    </Show>
                    <Show condition={listing}>
                        <Sandboxes state />
                    </Show>
                </List>
            </Frame>
        </Scroll>
    }
}

#[component]
fn OnTheWeb() -> NodeId {
    view! {
        <List spacing=SPACING @test_id="boat.web">
            <List direction=Direction::Horizontal align=Align::Center spacing=SPACING>
                <Text style=TextStyle::HEADING>
                    <Icon glyph=ICON_GLOBE />
                </Text>
                <Text style=TextStyle::HEADING>"boat.dev is not available on the web"</Text>
            </List>
            <Text style=TextStyle::PARAGRAPH>
                "boat.dev answers only its own pages, so a browser keeps this page from reaching \
                 it. Open this block in the be3 app on a computer to see and start your sandboxes."
            </Text>
        </List>
    }
}
