use beui::reactive::{
    Align, Direction, Frame, ItemSize, List, Show, Text, clone, component, create_memo,
    create_signal, view,
};
use beui::styled::{Button, ButtonVariant, Icon, use_theme};
use beui::{NodeId, TextAlign};
use block_editor_plugin::{
    ChildBlock, ChildMode, ChildState, ChildTarget, InteractionMode,
    block_ui::{EMBEDDED_EDITOR_PADDING, EMBEDDED_EDITOR_TITLE_GAP, EMBEDDED_EDITOR_TITLE_HEIGHT},
};

use super::embeds::ResolvedEmbed;
use super::state::{FocusedEmbed, Shared};

const TITLE_SPACING: f32 = 6.0;
const TITLE_PADDING: f32 = 6.0;
const FRAME_RADIUS: u8 = 6;
const TITLE_RADIUS: u8 = 4;
const TITLE_FONT_SIZE: f32 = 16.0;
const UNAVAILABLE_FONT_SIZE: f32 = 13.0;

#[component]
pub(crate) fn LargeEmbed(state: Shared, embed: ResolvedEmbed) -> NodeId {
    let theme = use_theme();
    let key = FocusedEmbed {
        id: embed.id,
        source_start: embed.range.start,
    };
    let target = ChildTarget::new(embed.id, embed.block_type);
    let (child, set_child) = create_signal(ChildState::default());
    let focused_embed = state.focused_embed.clone();
    let focused = create_memo(clone!(focused_embed -> move || focused_embed.get() == Some(key)));
    let mode = create_memo(clone!(child focused -> move || {
        match (focused.get(), child.get().interaction) {
            (true, _) => ChildMode::Active,
            (false, Some(InteractionMode::Preview)) => ChildMode::Preview,
            (false, _) => ChildMode::Passive,
        }
    }));
    let available = embed.available;
    let editable = create_memo(clone!(child focused -> move || {
        available && child.get().interaction == Some(InteractionMode::Preview) && !focused.get()
    }));
    let unavailable = !available;
    let has_icon = embed.icon.is_some();
    let icon = embed.icon.unwrap_or_default().to_owned();
    let label = embed.label.clone();
    let automatic = embed.automatic;

    let report = state.clone();
    let edit_state = state.clone();
    let icon_theme = theme.clone();
    let icon_row = move || {
        let color = icon_theme.text.clone();
        let glyph = icon.clone();
        view! {
            <Icon glyph={glyph} color={color} />
        }
    };
    let unavailable_theme = theme.clone();
    let unavailable_row = move || {
        let color = unavailable_theme.text_muted.clone();
        view! {
            <Text
                string="Block unavailable"
                font_size=UNAVAILABLE_FONT_SIZE
                color={color}
                align=TextAlign::Center
            />
        }
    };
    view! {
        <Frame
            color={theme.surface.clone()}
            outline={theme.border.clone()}
            outline_width=1.0
            outline_visible=true
            radius=FRAME_RADIUS
            padding_horizontal=EMBEDDED_EDITOR_PADDING
            padding_vertical=EMBEDDED_EDITOR_PADDING
        >
            <List spacing=EMBEDDED_EDITOR_TITLE_GAP>
                <Frame
                    height=EMBEDDED_EDITOR_TITLE_HEIGHT
                    color={theme.surface_raised.clone()}
                    radius=TITLE_RADIUS
                    padding_horizontal=TITLE_PADDING
                >
                    <List direction=Direction::Horizontal align=Align::Center spacing=TITLE_SPACING>
                        <Show condition={has_icon} then={icon_row} />
                        <Text
                            @sizing=ItemSize::Percent(100.0)
                            string={label}
                            font_size=TITLE_FONT_SIZE
                            italic={automatic}
                            color={theme.text.clone()}
                            clip=true
                        />
                        <Show condition={editable}>
                            {move || view! {
                                <Button
                                    label="Edit"
                                    variant=ButtonVariant::Secondary
                                    @test_id={format!("text.embed.{}.edit", key.id)}
                                    on_click={clone!(edit_state -> move || {
                                        edit_state.set_focused_embed.set(Some(key));
                                        edit_state.focus_confirmed.set(false);
                                    })}
                                />
                            }}
                        </Show>
                    </List>
                </Frame>
                <Show condition={unavailable} then={unavailable_row} />
                <Show condition={available}>
                    {move || view! {
                        <ChildBlock
                            @sizing=ItemSize::Percent(100.0)
                            editor={report.editor.clone()}
                            block={Some(target)}
                            mode={mode.clone()}
                            on_state={clone!(report -> move |next: ChildState| {
                                set_child.set(next.clone());
                                report.embed_children.borrow_mut().insert(key, next.clone());
                                if let Some(size) = next.intrinsic_size
                                    && report
                                        .embed_sizes
                                        .with_untracked(|sizes| sizes.get(&key.id) != Some(&size))
                                {
                                    report.set_embed_sizes.update(|sizes| {
                                        sizes.insert(key.id, size);
                                    });
                                }
                                confirm_focus(&report, key, next.active);
                            })}
                        />
                    }}
                </Show>
            </List>
        </Frame>
    }
}

fn confirm_focus(state: &Shared, key: FocusedEmbed, active: bool) {
    if state.focused_embed.get_untracked() != Some(key) {
        return;
    }
    if active {
        state.focus_confirmed.set(true);
        return;
    }
    if state.focus_confirmed.get() {
        state.set_focused_embed.set(None);
        state.focus_confirmed.set(false);
    }
}

pub(crate) fn embed_is_live(state: &Shared, key: FocusedEmbed) -> bool {
    state
        .embed_children
        .borrow()
        .get(&key)
        .is_some_and(|child| {
            matches!(child.interaction, Some(interaction) if interaction != InteractionMode::Preview)
        })
}
