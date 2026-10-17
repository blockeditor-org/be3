use crate::frame::Frame;
use beui_macros::{component, view};

use crate::button::{Button, ButtonVariant};
use crate::text::{Icon, Text, TextColor, TextSize, TextStyle};
use beui_core::font::TextAlign;
use beui_core::icons::{ICON_DELETE, ICON_HISTORY, ICON_RESTART_ALT};
use beui_core::node::NodeId;
use beui_view::primitives::*;
use beui_view::reactive::*;

const PADDING: f32 = 16.0;
const SPACING: f32 = 8.0;

#[component]
pub fn StaleNotice(
    title: Prop<String>,
    detail: Prop<String>,
    #[prop(default = true)] can_try: Prop<bool>,
    #[prop(default = "stale-notice".to_owned())] id: String,
    on_reset: ClickCallback,
    on_delete: ClickCallback,
    on_try: ClickCallback,
) -> NodeId {
    let (reset_id, delete_id, try_id) = (
        format!("{id}.reset"),
        format!("{id}.delete"),
        format!("{id}.try"),
    );
    view! {
        <Frame @test_id={id} padding=PADDING>
            <List spacing=SPACING align=Align::Center>
                <Text size=TextSize::Xl color=TextColor::Muted align=TextAlign::Center>
                    <Icon glyph={ICON_HISTORY.to_owned()} />
                </Text>
                <Text style=TextStyle::HEADING string={title} align=TextAlign::Center />
                <Text
                    string={create_memo(move || detail.get())}
                    size=TextSize::Sm
                    color=TextColor::Muted
                    align=TextAlign::Center
                    wrap=true
                />
                <List direction=Direction::Horizontal align=Align::Center spacing=SPACING>
                    <Button
                        @test_id={reset_id}
                        label="Reset"
                        glyph={ICON_RESTART_ALT.to_owned()}
                        variant=ButtonVariant::Primary
                        on_click={on_reset}
                    />
                    <Button
                        @test_id={delete_id}
                        label="Delete"
                        glyph={ICON_DELETE.to_owned()}
                        variant=ButtonVariant::Secondary
                        on_click={on_delete}
                    />
                    <Show condition={create_memo(move || can_try.get())}>|| {
                        let try_id = try_id.clone(); view! {
                            <Button
                                @test_id={try_id}
                                label="Try anyway"
                                variant=ButtonVariant::Ghost
                                on_click={on_try}
                            />
                        }
                    }</Show>
                </List>
            </List>
        </Frame>
    }
}
