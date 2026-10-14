use beui_macros::{component, view};

use crate::button::{Button, ButtonVariant};
use crate::text::{Heading, Icon};
use crate::theme::{FONT_SMALL, use_theme};
use beui_core::font::TextAlign;
use beui_core::icons::{ICON_DELETE, ICON_HISTORY, ICON_RESTART_ALT};
use beui_core::node::NodeId;
use beui_view::reactive::{
    Align, ClickCallback, Direction, Frame, List, Prop, Show, Text, clone, create_memo,
};

const PADDING: f32 = 16.0;
const SPACING: f32 = 8.0;
const GLYPH_SIZE: f32 = 28.0;

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
    let theme = use_theme();
    let (reset_id, delete_id, try_id) = (
        format!("{id}.reset"),
        format!("{id}.delete"),
        format!("{id}.try"),
    );
    view! {
        <Frame @test_id={id} padding_horizontal=PADDING padding_vertical=PADDING>
            <List spacing=SPACING align=Align::Center>
                <Icon
                    glyph={ICON_HISTORY.to_owned()}
                    text_size=GLYPH_SIZE
                    color={theme.text_muted}
                />
                <Heading content={title} align=TextAlign::Center />
                <Text
                    string={create_memo(move || detail.get())}
                    font_size=FONT_SMALL
                    color={theme.text_muted}
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
                    <Show condition={create_memo(move || can_try.get())}>
                        {move || clone!(try_id -> view! {
                            <Button
                                @test_id={try_id}
                                label="Try anyway"
                                variant=ButtonVariant::Ghost
                                on_click={on_try}
                            />
                        })}
                    </Show>
                </List>
            </List>
        </Frame>
    }
}
