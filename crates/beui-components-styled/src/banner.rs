use beui_components_unstyled as unstyled;
use beui_core::node::NodeId;
use beui_macros::{component, view};
use beui_view::reactive::*;

use crate::button::{Button, ButtonSize, ButtonVariant};
use crate::frame::{ColorRole, Frame};
use crate::list::List;
use crate::text::{Text, TextColor};
use crate::tokens::{Padding, Space};

#[component]
pub fn Banner(
    message: Prop<Option<String>>,
    #[prop(default = String::from("banner"))] id: Prop<String>,
    #[prop(default = String::from("Dismiss"))] dismiss_label: Prop<String>,
    on_dismiss: ClickCallback,
) -> NodeId {
    let message = create_memo(move || message.get());
    let shown = create_memo(move || message.with(Option::is_some));
    let text = create_memo(move || message.get().unwrap_or_default());
    let id = id.get_untracked();
    view! {
        <List spacing=Space::None>
            <Show condition={shown}>
                <BannerBar
                    text
                    id={id.clone()}
                    dismiss_label={dismiss_label.clone()}
                    on_dismiss={on_dismiss.clone()}
                />
            </Show>
        </List>
    }
}

#[component]
fn BannerBar(
    text: Memo<String>,
    id: String,
    dismiss_label: Prop<String>,
    on_dismiss: ClickCallback,
) -> NodeId {
    let dismissable = !on_dismiss.is_empty();
    let dismiss_id = format!("{id}.dismiss");
    view! {
        <unstyled::Banner label={text} @test_id={id}>
            <Frame fill=ColorRole::DangerSoft padding={Padding::symmetric(Space::Lg, Space::Sm)}>
                <List direction=Direction::Horizontal align=Align::Center spacing=Space::Md>
                    <Text @sizing={ItemSize::Percent(100.0)} color=TextColor::Danger>{text}</Text>
                    <Show condition=dismissable>
                        <Button
                            label={dismiss_label.clone()}
                            size=ButtonSize::Compact
                            variant=ButtonVariant::Secondary
                            @test_id={dismiss_id.clone()}
                            on_click={on_dismiss.clone()}
                        />
                    </Show>
                </List>
            </Frame>
        </unstyled::Banner>
    }
}
