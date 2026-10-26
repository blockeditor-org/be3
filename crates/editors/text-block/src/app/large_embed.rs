use beui::reactive::*;
use beui::styled::*;
use beui::{NodeId, Overflow, TextAlign};
use block_editor_beui::{
    ChildBlock, ChildMode, ChildState, ChildTarget, InteractionMode,
    block_ui::{EMBEDDED_EDITOR_PADDING, EMBEDDED_EDITOR_TITLE_GAP, EMBEDDED_EDITOR_TITLE_HEIGHT},
};

use super::embeds::ResolvedEmbed;
use super::state::{FocusedEmbed, State};

const TITLE_SPACING: f32 = 6.0;
const TITLE_PADDING: f32 = 6.0;
const FRAME_RADIUS: f32 = 6.0;
const TITLE_RADIUS: f32 = 4.0;

#[component]
pub(crate) fn LargeEmbed(state: State, embed: ResolvedEmbed) -> NodeId {
    let key = FocusedEmbed {
        id: embed.id,
        source_start: embed.range.start,
    };
    let target = ChildTarget::new(embed.id, embed.block_type);
    let child = create_signal(ChildState::default());
    let focused_embed = state.focused_embed;
    let focused = create_memo(move || focused_embed.get() == Some(key));
    let mode = create_memo(move || match (focused.get(), child.get().interaction) {
        (true, _) => ChildMode::Active,
        (false, Some(InteractionMode::Preview)) => ChildMode::Preview,
        (false, _) => ChildMode::Passive,
    });
    let available = embed.available;
    let editable = create_memo(move || {
        available && child.get().interaction == Some(InteractionMode::Preview) && !focused.get()
    });
    let unavailable = !available;
    let has_icon = embed.icon.is_some();
    let icon = embed.icon;
    let label = embed.label.clone();
    let automatic = embed.automatic;
    let deleted = embed.deleted;

    let icon_row = move || {
        let glyph = icon;
        view! {
            <Text align=TextAlign::Center>
                <Icon glyph={glyph} />
            </Text>
        }
    };
    let unavailable_row = move || {
        view! {
            <Text size=TextSize::Sm color=TextColor::Muted align=TextAlign::Center>
                "Block unavailable"
            </Text>
        }
    };
    view! {
        <Frame
            fill=ColorRole::Surface
            border={Border::new(ColorRole::Border).width(1.0)}
            radius={Radius::custom(FRAME_RADIUS)}
            padding=EMBEDDED_EDITOR_PADDING
        >
            <List spacing=EMBEDDED_EDITOR_TITLE_GAP>
                <Frame
                    height=EMBEDDED_EDITOR_TITLE_HEIGHT
                    fill=ColorRole::SurfaceRaised
                    radius={Radius::custom(TITLE_RADIUS)}
                    padding={Sides::horizontal(TITLE_PADDING)}
                >
                    <List direction=Direction::Horizontal align=Align::Center spacing=TITLE_SPACING>
                        <Show condition={has_icon} then={icon_row} />
                        <Text
                            @sizing=ItemSize::Percent(100.0)
                            size=TextSize::Lg
                            italic={automatic}
                            overflow=Overflow::Ellipsis
                        >
                            {label}
                        </Text>
                        <Show condition={deleted}>
                            <Text size=TextSize::Sm color=TextColor::Muted>
                                "In Recently Deleted"
                            </Text>
                        </Show>
                        <Show condition={editable}>
                            <Button
                                label="Edit"
                                variant=ButtonVariant::Secondary
                                @test_id={format!("text.embed.{}.edit", key.id)}
                                on_click={move || {
                                    state.focused_embed.set(Some(key));
                                    state.inner().focus_confirmed.set(false);
                                }}
                            />
                        </Show>
                    </List>
                </Frame>
                <Show condition={unavailable} then={unavailable_row} />
                <Show condition={available}>
                    <ChildBlock
                        @sizing=ItemSize::Percent(100.0)
                        editor={state.editor}
                        block={Some(target)}
                        mode={mode}
                        on_state={move |next: ChildState| {
                            child.set(next.clone());
                            state.inner().embed_children.borrow_mut().insert(key, next.clone());
                            if let Some(size) = next.intrinsic_size
                                && state
                                    .embed_sizes
                                    .with_untracked(|sizes| sizes.get(&key.id) != Some(&size))
                            {
                                state.embed_sizes.update(|sizes| {
                                    sizes.insert(key.id, size);
                                });
                            }
                            confirm_focus(state, key, next.active);
                        }}
                    />
                </Show>
            </List>
        </Frame>
    }
}

fn confirm_focus(state: State, key: FocusedEmbed, active: bool) {
    if state.focused_embed.get_untracked() != Some(key) {
        return;
    }
    if active {
        state.inner().focus_confirmed.set(true);
        return;
    }
    if state.inner().focus_confirmed.get() {
        state.focused_embed.set(None);
        state.inner().focus_confirmed.set(false);
    }
}

pub(crate) fn embed_is_live(state: State, key: FocusedEmbed) -> bool {
    state
        .inner()
        .embed_children
        .borrow()
        .get(&key)
        .is_some_and(|child| {
            matches!(child.interaction, Some(interaction) if interaction != InteractionMode::Preview)
        })
}
