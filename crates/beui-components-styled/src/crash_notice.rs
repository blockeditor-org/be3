use beui_macros::{component, view};

use crate::button::{Button, ButtonVariant};
use crate::text::{Heading, Icon};
use crate::theme::{FONT_SMALL, use_theme};
use beui_core::font::TextAlign;
use beui_core::icons::{ICON_ERROR, ICON_REFRESH};
use beui_core::node::NodeId;
use beui_view::reactive::{
    Align, ClickCallback, Frame, List, Prop, Show, Text, clone, create_memo,
};

const PADDING: f32 = 16.0;
const SPACING: f32 = 8.0;
const GLYPH_SIZE: f32 = 28.0;

#[component]
pub fn CrashNotice(
    title: Prop<String>,
    detail: Prop<String>,
    #[prop(default = true)] reloadable: Prop<bool>,
    #[prop(default = "Reload".to_owned())] reload_label: Prop<String>,
    #[prop(default = "crash-notice".to_owned())] id: String,
    on_reload: ClickCallback,
) -> NodeId {
    let reload_id = format!("{id}.reload");
    let theme = use_theme();
    let reloadable = create_memo(move || reloadable.get());
    let has_detail = create_memo(clone!(detail -> move || !detail.get().is_empty()));
    view! {
        <Frame @test_id={id} padding_horizontal=PADDING padding_vertical=PADDING>
            <List spacing=SPACING align=Align::Center>
                <Icon
                    glyph={ICON_ERROR.to_owned()}
                    text_size=GLYPH_SIZE
                    color={theme.danger.clone()}
                />
                <Heading content={title} align=TextAlign::Center />
                <Show condition={has_detail}>
                    <Text
                        string={create_memo(clone!(detail -> move || detail.get()))}
                        font_size=FONT_SMALL
                        color={theme.text_muted.clone()}
                        align=TextAlign::Center
                        wrap=true
                    />
                </Show>
                <Show condition={reloadable}>
                    {move || clone!(on_reload reload_label reload_id -> view! {
                        <Button
                            @test_id={reload_id}
                            label={reload_label}
                            glyph={ICON_REFRESH.to_owned()}
                            variant=ButtonVariant::Secondary
                            on_click={move || on_reload.call()}
                        />
                    })}
                </Show>
            </List>
        </Frame>
    }
}
