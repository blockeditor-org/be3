use crate::frame::{ColorRole, Frame};
use beui_macros::{component, view};

use text_editor_core::FindDirection;

use crate::Button;
use crate::ButtonVariant;
use crate::Icon;
use crate::TextInput;
use crate::ToggleButton;
use crate::text::{Text, TextColor, TextStyle};
use beui_components_unstyled::TextAreaState;
use beui_view::primitives::BackHandler;
use beui_view::reactive::NodeId;
use beui_view::types::TextAlign;
use beui_view::types::icons::{
    ICON_CASE_SENSITIVE, ICON_CHEVRON_DOWN, ICON_CHEVRON_UP, ICON_SEARCH, ICON_X,
};

use crate::list::List;
use crate::tokens::{Padding, Size, Space};
use beui_view::reactive::*;
use beui_view::types::{Align, Direction};

const BAR_PADDING: Padding = Padding::symmetric(Space::Lg, Space::Sm);
const BAR_SPACING: Space = Space::Sm;
const QUERY_WIDTH: Size = Size::Custom(180.0);

#[component]
pub fn FindBar(state: TextAreaState, #[prop(default = false)] read_only: Prop<bool>) -> NodeId {
    let open = state.find_open();
    let read_only = create_memo(move || read_only.get());
    view! {
        <BackHandler enabled={open} on_back={move || state.close_find()}>
            <List spacing=Space::None>
                <Show condition={open}>
                    <FindBarRows state={state} read_only />
                </Show>
            </List>
        </BackHandler>
    }
}

#[component]
fn FindBarRows(state: TextAreaState, read_only: Memo<bool>) -> NodeId {
    let summary = create_memo(move || state.find_summary());
    let no_matches = create_memo(move || !state.find_has_matches());
    let no_current = create_memo(move || !state.find_has_current());

    let query = state.find().query;
    let case_sensitive = state.find().case_sensitive;
    let focus_query = state.find().focus_query;
    let show_replace = state.find().show_replace;
    let show_replace = create_memo(move || show_replace.get() && !read_only.get());
    view! {
        <Frame fill=ColorRole::Surface>
            <List spacing=Space::None>
                <Frame padding=BAR_PADDING>
                    <List direction=Direction::Horizontal align=Align::Center spacing=BAR_SPACING>
                        <Text color=TextColor::Muted align=TextAlign::Center>
                            <Icon glyph=ICON_SEARCH />
                        </Text>
                        <Frame width=QUERY_WIDTH>
                            <TextInput
                                value={Binding::new(query, move |value: String| state.set_find_query(value))}
                                placeholder="Find"
                                label="Find"
                                focused={focus_query}
                                @test_id={"text.find.query"}
                                on_submit={move |_: String| state.find_step(FindDirection::Next)}
                            />
                        </Frame>
                        <ToggleButton
                            glyph=ICON_CASE_SENSITIVE
                            icon_only=true
                            label="Match case"
                            pressed={Binding::new(case_sensitive, move |pressed: bool| state.set_find_case_sensitive(pressed))}
                            @test_id={"text.find.match-case"}
                        />
                        <Text style=TextStyle::CAPTION>{summary}</Text>
                        <Button
                            icon_only=true
                            glyph=ICON_CHEVRON_UP
                            label="Previous match"
                            disabled={no_matches}
                            @test_id={"text.find.previous"}
                            on_click={move || state.find_step(FindDirection::Previous)}
                        />
                        <Button
                            icon_only=true
                            glyph=ICON_CHEVRON_DOWN
                            label="Next match"
                            disabled={no_matches}
                            @test_id={"text.find.next"}
                            on_click={move || state.find_step(FindDirection::Next)}
                        />
                        <Button
                            icon_only=true
                            glyph=ICON_X
                            label="Close"
                            @test_id={"text.find.close"}
                            on_click={move || state.close_find()}
                        />
                    </List>
                </Frame>
                <Show condition={show_replace}>
                    <ReplaceRow state={state} no_current={no_current} no_matches={no_matches} />
                </Show>
            </List>
        </Frame>
    }
}

#[component]
fn ReplaceRow(state: TextAreaState, no_current: Memo<bool>, no_matches: Memo<bool>) -> NodeId {
    let replacement = state.find().replacement;
    view! {
        <Frame padding=BAR_PADDING>
            <List direction=Direction::Horizontal align=Align::Center spacing=BAR_SPACING>
                <Frame width=QUERY_WIDTH>
                    <TextInput
                        value={Binding::new(replacement, move |value: String| state.set_find_replacement(value))}
                        placeholder="Replace"
                        label="Replace"
                        @test_id={"text.find.replacement"}
                    />
                </Frame>
                <Button
                    label="Replace"
                    variant=ButtonVariant::Secondary
                    disabled={no_current}
                    @test_id={"text.find.replace"}
                    on_click={move || state.replace_match()}
                />
                <Button
                    label="Replace All"
                    variant=ButtonVariant::Secondary
                    disabled={no_matches}
                    @test_id={"text.find.replace-all"}
                    on_click={move || state.replace_all_matches()}
                />
            </List>
        </Frame>
    }
}
