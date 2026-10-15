use beui_macros::{component, view};

use crate::button::{Button, ButtonVariant};
use crate::text::{Icon, Text, TextColor, TextSize, TextStyle};
use beui_core::font::TextAlign;
use beui_core::icons::{ICON_ERROR, ICON_REFRESH};
use beui_core::node::NodeId;
use beui_view::primitives::*;
use beui_view::reactive::*;

const PADDING: f32 = 16.0;
const SPACING: f32 = 8.0;

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
    let reloadable = create_memo(move || reloadable.get());
    let has_detail = create_memo({
        let detail = detail.clone();
        move || !detail.get().is_empty()
    });
    view! {
        <Frame @test_id={id} padding=PADDING>
            <List spacing=SPACING align=Align::Center>
                <Text size=TextSize::Xl color=TextColor::Danger align=TextAlign::Center>
                    <Icon glyph={ICON_ERROR.to_owned()} />
                </Text>
                <Text style=TextStyle::HEADING string={title} align=TextAlign::Center />
                <Show condition={has_detail}>
                    <Text
                        string={create_memo({ let detail = detail.clone(); move || detail.get() })}
                        size=TextSize::Sm
                        color=TextColor::Muted
                        align=TextAlign::Center
                        wrap=true
                    />
                </Show>
                <Show condition={reloadable}>
                    {move || { let reload_label = reload_label.clone(); let reload_id = reload_id.clone(); view! {
                        <Button
                            @test_id={reload_id}
                            label={reload_label}
                            glyph={ICON_REFRESH.to_owned()}
                            variant=ButtonVariant::Secondary
                            on_click={on_reload}
                        />
                    } }}
                </Show>
            </List>
        </Frame>
    }
}
