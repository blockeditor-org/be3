use beui::NodeId;
use beui::icons::{
    ICON_CLOSE, ICON_KEYBOARD_ARROW_DOWN, ICON_KEYBOARD_ARROW_UP, ICON_MATCH_CASE, ICON_SEARCH,
};
use beui::reactive::{
    Align, Direction, Frame, List, Memo, ReadSignal, Show, WriteSignal, clone, component,
    create_memo, create_signal, view,
};
use beui::styled::{
    Body, Button, ButtonVariant, Caption, Icon, IconButton, TextInput, ToggleButton, use_theme,
};
use text_editor_core::{CopyMode, EditorCommand, FindDirection};

use super::state::{Shared, State};

const BAR_PADDING_HORIZONTAL: f32 = 12.0;
const BAR_PADDING_VERTICAL: f32 = 6.0;
const BAR_SPACING: f32 = 6.0;
const QUERY_WIDTH: f32 = 180.0;

pub(crate) struct Find {
    pub open: ReadSignal<bool>,
    pub set_open: WriteSignal<bool>,
    pub query: ReadSignal<String>,
    pub set_query: WriteSignal<String>,
    pub replace: ReadSignal<String>,
    pub set_replace: WriteSignal<String>,
    pub case_sensitive: ReadSignal<bool>,
    pub set_case_sensitive: WriteSignal<bool>,
    pub show_replace: ReadSignal<bool>,
    pub set_show_replace: WriteSignal<bool>,
    pub focus_query: ReadSignal<bool>,
    pub set_focus_query: WriteSignal<bool>,
}

impl Find {
    pub fn new() -> Self {
        let (open, set_open) = create_signal(false);
        let (query, set_query) = create_signal(String::new());
        let (replace, set_replace) = create_signal(String::new());
        let (case_sensitive, set_case_sensitive) = create_signal(false);
        let (show_replace, set_show_replace) = create_signal(false);
        let (focus_query, set_focus_query) = create_signal(false);
        Self {
            open,
            set_open,
            query,
            set_query,
            replace,
            set_replace,
            case_sensitive,
            set_case_sensitive,
            show_replace,
            set_show_replace,
            focus_query,
            set_focus_query,
        }
    }
}

pub(crate) fn open_find(state: &Shared, show_replace: bool) {
    if !state.find.open.get_untracked() {
        let selected = state.copy(CopyMode::Copy);
        let query = match !selected.is_empty() && !selected.contains('\n') {
            true => selected,
            false => String::new(),
        };
        state.find.set_query.set(query);
        state.find.set_open.set(true);
    }
    state.find.set_show_replace.set(show_replace);
    state.find.set_focus_query.set(true);
    if sync_find(state) {
        state.reveal_cursor.set(true);
    }
}

pub(crate) fn close_find(state: &State) {
    state.find.set_open.set(false);
    state.find.set_focus_query.set(false);
}

fn query(state: &State) -> (String, bool) {
    (
        state.find.query.get_untracked(),
        state.find.case_sensitive.get_untracked(),
    )
}

pub(crate) fn sync_find(state: &State) -> bool {
    if !state.find.open.get_untracked() {
        return false;
    }
    let (text, case_sensitive) = query(state);
    let status = state.core.borrow().find_status(&text, case_sensitive);
    if status.current.is_none() && status.total > 0 {
        state.execute(EditorCommand::Find {
            text: &text,
            case_sensitive,
            direction: FindDirection::Next,
        });
    }
    status.total > 0
}

pub(crate) fn find_step(state: &State, direction: FindDirection) -> bool {
    if !state.find.open.get_untracked() {
        return false;
    }
    let (text, case_sensitive) = query(state);
    state.execute(EditorCommand::Find {
        text: &text,
        case_sensitive,
        direction,
    });
    state.core.borrow().find_status(&text, case_sensitive).total > 0
}

fn replace_current(state: &State) {
    let (text, case_sensitive) = query(state);
    let replacement = state.find.replace.get_untracked();
    state.execute(EditorCommand::ReplaceMatch {
        text: &text,
        case_sensitive,
        replacement: replacement.as_bytes(),
    });
}

fn replace_all(state: &State) {
    let (text, case_sensitive) = query(state);
    let replacement = state.find.replace.get_untracked();
    state.execute(EditorCommand::ReplaceAllMatches {
        text: &text,
        case_sensitive,
        replacement: replacement.as_bytes(),
    });
}

#[component]
pub(crate) fn FindBar(state: Shared) -> NodeId {
    let open = state.find.open.clone();
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
fn FindBarRows(state: Shared) -> NodeId {
    let theme = use_theme();
    let content = state.content.clone();
    let cursors = state.cursors.clone();
    let query = state.find.query.clone();
    let case_sensitive = state.find.case_sensitive.clone();
    let focus_query = state.find.focus_query.clone();
    let show_replace = state.find.show_replace.clone();
    let status = create_memo(
        clone!(state content cursors query case_sensitive -> move || {
            content.get();
            cursors.get();
            let text = query.get();
            let case_sensitive = case_sensitive.get();
            let status = state.core.borrow().find_status(&text, case_sensitive);
            (text.is_empty(), status.total, status.current)
        }),
    );
    let summary = create_memo(clone!(status -> move || match status.get() {
        (true, 0, _) => String::new(),
        (false, 0, _) => "No results".to_owned(),
        (_, total, Some(current)) => format!("{}/{total}", current + 1),
        (_, total, None) => format!("0/{total}"),
    }));
    let no_matches = create_memo(clone!(status -> move || status.get().1 == 0));
    let no_current = create_memo(clone!(status -> move || status.get().2.is_none()));

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
                                value={query.clone()}
                                placeholder="Find"
                                label="Find"
                                focused={focus_query}
                                @test_id={"text.find.query"}
                                on_change={move |value: String| {
                                    change_state.find.set_query.set(value);
                                    if sync_find(&change_state) {
                                        change_state.reveal_cursor.set(true);
                                    }
                                }}
                                on_submit={move |_: String| {
                                    if find_step(&submit_state, FindDirection::Next) {
                                        submit_state.reveal_cursor.set(true);
                                    }
                                }}
                            />
                        </Frame>
                        <ToggleButton
                            glyph=ICON_MATCH_CASE
                            icon_only=true
                            label="Match case"
                            pressed={case_sensitive.clone()}
                            @test_id={"text.find.match-case"}
                            on_change={move |pressed: bool| {
                                case_state.find.set_case_sensitive.set(pressed);
                                if sync_find(&case_state) {
                                    case_state.reveal_cursor.set(true);
                                }
                            }}
                        />
                        <Caption content={summary} />
                        <IconButton
                            glyph=ICON_KEYBOARD_ARROW_UP
                            label="Previous match"
                            disabled={previous_disabled}
                            @test_id={"text.find.previous"}
                            on_click={move || {
                                if find_step(&previous_state, FindDirection::Previous) {
                                    previous_state.reveal_cursor.set(true);
                                }
                            }}
                        />
                        <IconButton
                            glyph=ICON_KEYBOARD_ARROW_DOWN
                            label="Next match"
                            disabled={no_matches}
                            @test_id={"text.find.next"}
                            on_click={move || {
                                if find_step(&next_state, FindDirection::Next) {
                                    next_state.reveal_cursor.set(true);
                                }
                            }}
                        />
                        <IconButton
                            glyph=ICON_CLOSE
                            label="Close"
                            @test_id={"text.find.close"}
                            on_click={move || close_find(&close_state)}
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
fn ReplaceRow(state: Shared, no_current: Memo<bool>, no_matches: Memo<bool>) -> NodeId {
    let replacement = state.find.replace.clone();
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
                        on_change={move |value: String| {
                            change_state.find.set_replace.set(value);
                        }}
                    />
                </Frame>
                <Button
                    label="Replace"
                    variant=ButtonVariant::Secondary
                    disabled={no_current}
                    @test_id={"text.find.replace"}
                    on_click={move || {
                        replace_current(&replace_state);
                        replace_state.reveal_cursor.set(true);
                    }}
                />
                <Button
                    label="Replace All"
                    variant=ButtonVariant::Secondary
                    disabled={no_matches}
                    @test_id={"text.find.replace-all"}
                    on_click={move || replace_all(&all_state)}
                />
            </List>
        </Frame>
    }
}

#[component]
pub(crate) fn ImportError(state: Shared) -> NodeId {
    let error = state.import_error.clone();
    let shown = create_memo(clone!(error -> move || error.get().is_some()));
    view! {
        <List spacing=0.0>
            <Show condition={shown}>
                {move || view! {
                    <ImportErrorRow state={state.clone()} />
                }}
            </Show>
        </List>
    }
}

#[component]
fn ImportErrorRow(state: Shared) -> NodeId {
    let theme = use_theme();
    let error = state.import_error.clone();
    let message = create_memo(clone!(error -> move || error.get().unwrap_or_default()));
    view! {
        <Frame
            color={theme.surface.clone()}
            padding_horizontal=BAR_PADDING_HORIZONTAL
            padding_vertical=BAR_PADDING_VERTICAL
        >
            <List direction=Direction::Horizontal align=Align::Center spacing=BAR_SPACING>
                <Body content={message} color={theme.danger.clone()} />
                <Button
                    label="Dismiss"
                    variant=ButtonVariant::Secondary
                    @test_id={"text.image-error.dismiss"}
                    on_click={move || state.set_import_error.set(None)}
                />
            </List>
        </Frame>
    }
}
