use beui::NodeId;
use beui::icons::ICON_CHEVRON_RIGHT;
use beui::reactive::{
    CenteredRow, ClickCallback, Column, Dynamic, Frame, ItemSize, Prop, Text, clone, component,
    create_memo, intrinsic, view,
};
use beui::styled::{Button, ButtonVariant, Icon, Separator, use_theme};
use beui::{Context, Key};

const BAND_PADDING_H: f32 = 12.0;
const BAND_PADDING_V: f32 = 8.0;
const STEP_SPACING: f32 = 6.0;

#[component]
pub(crate) fn TopBar(
    trail: Prop<Vec<String>>,
    shown: Prop<bool>,
    on_exit: ClickCallback,
) -> NodeId {
    let theme = use_theme();
    let trail = create_memo(move || trail.get());
    let visible = create_memo(clone!(trail -> move || shown.get() && !trail.get().is_empty()));
    view! {
        <Frame visible={visible} color={theme.surface.clone()}>
            <Column spacing=0.0>
                <Frame padding_horizontal=BAND_PADDING_H padding_vertical=BAND_PADDING_V>
                    <CenteredRow spacing=12.0>
                        <CenteredRow @sizing=ItemSize::Percent(100.0) spacing=STEP_SPACING>
                            <Dynamic value={trail}>
                                {move |trail: Vec<String>| breadcrumb(trail)}
                            </Dynamic>
                        </CenteredRow>
                        <Button
                            label="Close"
                            variant=ButtonVariant::Secondary
                            on_click={move || on_exit.call()}
                            @test_id={"editor.close"}
                        />
                    </CenteredRow>
                </Frame>
                <Separator />
            </Column>
        </Frame>
    }
}

fn breadcrumb(trail: Vec<String>) -> NodeId {
    let theme = use_theme();
    let last_index = trail.len().saturating_sub(1);
    let mut children = Vec::new();
    for (index, step) in trail.into_iter().enumerate() {
        if index > 0 {
            children.push(intrinsic(view! {
                <Icon
                    glyph={ICON_CHEVRON_RIGHT.to_owned()}
                    color={theme.text_muted.clone()}
                />
            }));
        }
        let color = match index == last_index {
            true => theme.text.clone(),
            false => theme.text_muted.clone(),
        };
        children.push(intrinsic(view! {
            <Text string={step} color={color} />
        }));
    }
    view! {
        <CenteredRow spacing=STEP_SPACING children={children} />
    }
}

pub(crate) fn escaped(context: &Context) -> bool {
    context.input(|input| {
        input.events.iter().any(|event| {
            matches!(
                event,
                beui::Event::Key {
                    key: Key::Escape,
                    pressed: true,
                    ..
                }
            )
        })
    })
}
