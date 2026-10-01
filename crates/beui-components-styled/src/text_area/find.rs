use beui_macros::{component, view};

use text_editor_core::{FindDirection, FindStatus};

use crate::Button;
use crate::ButtonVariant;
use crate::Caption;
use crate::Icon;
use crate::IconButton;
use crate::TextInput;
use crate::ToggleButton;
use crate::use_theme;
use beui_components_unstyled::TextAreaState;
use beui_core::icons::{
    ICON_CLOSE, ICON_KEYBOARD_ARROW_DOWN, ICON_KEYBOARD_ARROW_UP, ICON_MATCH_CASE, ICON_SEARCH,
};
use beui_core::node::NodeId;
use beui_view::reactive::{Align, Direction, Frame, List, Memo, Show, clone, create_memo};

const BAR_PADDING_HORIZONTAL: f32 = 12.0;
const BAR_PADDING_VERTICAL: f32 = 6.0;
const BAR_SPACING: f32 = 6.0;
const QUERY_WIDTH: f32 = 180.0;

#[component]
pub fn FindBar(state: TextAreaState) -> NodeId {
    let open = state.find_open();
    view! {
        <List spacing=0.0>
            <Show condition={open}>
                {move || view! {
                    <FindBarRows state={state.clone()} />
                }}
            </Show>
        </List>
    }
}

#[component]
fn FindBarRows(state: TextAreaState) -> NodeId {
    let theme = use_theme();
    let status = create_memo(clone!(state -> move || {
        let query_empty = state.find().query.get().is_empty();
        (query_empty, state.find_status())
    }));
    let summary = create_memo(clone!(status -> move || match status.get() {
        (true, FindStatus { total: 0, .. }) => String::new(),
        (false, FindStatus { total: 0, .. }) => "No results".to_owned(),
        (_, FindStatus { total, current: Some(current) }) => format!("{}/{total}", current + 1),
        (_, FindStatus { total, current: None }) => format!("0/{total}"),
    }));
    let no_matches = create_memo(clone!(status -> move || status.get().1.total == 0));
    let no_current = create_memo(clone!(status -> move || status.get().1.current.is_none()));

    let query = state.find().query.clone();
    let case_sensitive = state.find().case_sensitive.clone();
    let focus_query = state.find().focus_query.clone();
    let show_replace = state.find().show_replace.clone();
    let change_state = state.clone();
    let submit_state = state.clone();
    let case_state = state.clone();
    let previous_state = state.clone();
    let next_state = state.clone();
    let close_state = state.clone();
    let replace_state = state.clone();
    let previous_disabled = no_matches.clone();
    let replace_no_matches = no_matches.clone();
    view! {
        <Frame color={theme.surface.clone()}>
            <List spacing=0.0>
                <Frame
                    padding_horizontal=BAR_PADDING_HORIZONTAL
                    padding_vertical=BAR_PADDING_VERTICAL
                >
                    <List direction=Direction::Horizontal align=Align::Center spacing=BAR_SPACING>
                        <Icon glyph=ICON_SEARCH color={theme.text_muted.clone()} />
                        <Frame width=QUERY_WIDTH>
                            <TextInput
                                value={query}
                                placeholder="Find"
                                label="Find"
                                focused={focus_query}
                                @test_id={"text.find.query"}
                                on_change={move |value: String| change_state.set_find_query(value)}
                                on_submit={move |_: String| submit_state.find_step(FindDirection::Next)}
                            />
                        </Frame>
                        <ToggleButton
                            glyph=ICON_MATCH_CASE
                            icon_only=true
                            label="Match case"
                            pressed={case_sensitive}
                            @test_id={"text.find.match-case"}
                            on_change={move |pressed: bool| case_state.set_find_case_sensitive(pressed)}
                        />
                        <Caption content={summary} />
                        <IconButton
                            glyph=ICON_KEYBOARD_ARROW_UP
                            label="Previous match"
                            disabled={previous_disabled}
                            @test_id={"text.find.previous"}
                            on_click={move || previous_state.find_step(FindDirection::Previous)}
                        />
                        <IconButton
                            glyph=ICON_KEYBOARD_ARROW_DOWN
                            label="Next match"
                            disabled={no_matches}
                            @test_id={"text.find.next"}
                            on_click={move || next_state.find_step(FindDirection::Next)}
                        />
                        <IconButton
                            glyph=ICON_CLOSE
                            label="Close"
                            @test_id={"text.find.close"}
                            on_click={move || close_state.close_find()}
                        />
                    </List>
                </Frame>
                <Show condition={show_replace}>
                    {move || view! {
                        <ReplaceRow
                            state={replace_state.clone()}
                            no_current={no_current.clone()}
                            no_matches={replace_no_matches.clone()}
                        />
                    }}
                </Show>
            </List>
        </Frame>
    }
}

#[component]
fn ReplaceRow(state: TextAreaState, no_current: Memo<bool>, no_matches: Memo<bool>) -> NodeId {
    let replacement = state.find().replacement.clone();
    let change_state = state.clone();
    let replace_state = state.clone();
    let all_state = state.clone();
    view! {
        <Frame padding_horizontal=BAR_PADDING_HORIZONTAL padding_vertical=BAR_PADDING_VERTICAL>
            <List direction=Direction::Horizontal align=Align::Center spacing=BAR_SPACING>
                <Frame width=QUERY_WIDTH>
                    <TextInput
                        value={replacement}
                        placeholder="Replace"
                        label="Replace"
                        @test_id={"text.find.replacement"}
                        on_change={move |value: String| change_state.set_find_replacement(value)}
                    />
                </Frame>
                <Button
                    label="Replace"
                    variant=ButtonVariant::Secondary
                    disabled={no_current}
                    @test_id={"text.find.replace"}
                    on_click={move || replace_state.replace_match()}
                />
                <Button
                    label="Replace All"
                    variant=ButtonVariant::Secondary
                    disabled={no_matches}
                    @test_id={"text.find.replace-all"}
                    on_click={move || all_state.replace_all_matches()}
                />
            </List>
        </Frame>
    }
}
