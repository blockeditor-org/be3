use crate::frame::Frame;
use beui_core::icons::Glyph;
use beui_core::layer::LayerLevel;
use std::rc::Rc;

use accesskit::{Node, Role};
use beui_macros::{component, view};

use crate::icon_button::IconButton;
use crate::text::{Text, TextColor, TextStyle};
use crate::text_input::TextInput;
use crate::theme::use_theme;
use beui_core::base::overlay::{OverlayAnchor, Placement};
use beui_core::geometry::Pos2;
use beui_core::node::NodeId;
use beui_view::components::overlay::Overlay;
use beui_view::primitives::*;
use beui_view::reactive::*;

const FIELD_WIDTH: f32 = 280.0;
const SPACING: f32 = 8.0;
const GROUP_SPACING: f32 = 28.0;
const ACTION_SPACING: f32 = 4.0;

#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub struct LockAction {
    pub label: String,
    pub glyph: Glyph,
}

#[component]
pub fn LockScreen(
    open: Prop<bool>,
    time: Prop<String>,
    date: Prop<String>,
    user: Prop<String>,
    #[prop(default = None)] error: Prop<Option<String>>,
    #[prop(default = false)] busy: Prop<bool>,
    #[prop(default = Vec::new())] actions: Prop<Vec<LockAction>>,
    #[prop(default = "lock".to_owned())] id: String,
    on_submit: Callback<String>,
    on_action: Callback<usize>,
) -> NodeId {
    let theme = use_theme();
    let open = create_memo(move || open.get());
    let time = create_memo(move || time.get());
    let date = create_memo(move || date.get());
    let user = create_memo(move || user.get());
    let error = create_memo(move || error.get());
    let busy = create_memo(move || busy.get());
    let actions = create_memo(move || actions.get());
    let password = create_signal(String::new());
    let submit = Rc::new(move |typed: String| {
        if busy.get_untracked() || typed.is_empty() {
            return;
        }
        password.set(String::new());
        on_submit.call(typed);
    });
    view! {
        <Overlay
            anchor=OverlayAnchor::Point(Pos2::ZERO)
            open={open}
            placement=Placement::Fill
            scrim={theme.background}
            level=LayerLevel::Lock
            content={move || {
                let submit = submit.clone();
                let focused = create_memo(move || open.get() && !busy.get());
                view! {
                    <LockCard
                        time={time}
                        date={date}
                        user={user}
                        error={error}
                        busy={busy}
                        actions={actions}
                        password={password}
                        focused={focused}
                        id={id.clone()}
                        on_submit={move |typed: String| submit(typed)}
                        on_action={on_action}
                    />
                }
            }}
        />
    }
}

#[component]
fn LockCard(
    time: Memo<String>,
    date: Memo<String>,
    user: Memo<String>,
    error: Memo<Option<String>>,
    busy: Memo<bool>,
    actions: Memo<Vec<LockAction>>,
    password: Binding<String>,
    focused: Memo<bool>,
    id: String,
    on_submit: Callback<String>,
    on_action: Callback<usize>,
) -> NodeId {
    component_accessibility(create_memo(|| {
        let mut node = Node::new(Role::Dialog);
        node.set_label("Locked");
        node
    }));
    let failed = create_memo(move || error.get().is_some());
    let checking = create_memo(move || busy.get() && error.get().is_none());
    let reason = create_memo(move || error.get().unwrap_or_default());
    let slots = create_memo(move || (0..actions.with(Vec::len)).collect::<Vec<usize>>());
    let error_id = format!("{id}.error");
    let action_id = id.clone();
    let card_id = id.clone();
    view! {
        <Frame align=Align::Center @test_id=card_id>
            <List spacing=GROUP_SPACING align=Align::Center>
                <List spacing=SPACING align=Align::Center>
                    <Text style=TextStyle::DISPLAY>{time}</Text>
                    <Text color=TextColor::Muted>{date}</Text>
                </List>
                <List spacing=SPACING align=Align::Center>
                    <Text style=TextStyle::TITLE>{user}</Text>
                    <Frame width=FIELD_WIDTH>
                        <TextInput
                            value={password}
                            placeholder="Password"
                            label="Password"
                            password=true
                            disabled={busy}
                            focused={focused}
                            @test_id={format!("{id}.password")}
                            on_submit={on_submit}
                        />
                    </Frame>
                    <Show condition={checking}>
                        <Text style=TextStyle::CAPTION>"Checking…"</Text>
                    </Show>
                    <Show condition={failed}>
                        <Text
                            style=TextStyle::CAPTION
                            color=TextColor::Danger
                            @test_id={error_id.clone()}
                        >
                            {reason}
                        </Text>
                    </Show>
                </List>
                <List direction=Direction::Horizontal spacing=ACTION_SPACING>
                    <ForEach keys={slots}>|index: usize| {
                        let glyph = create_memo(move || {
                            actions.with(|actions| actions.get(index).map(|action| action.glyph))
                        });
                        let label = create_memo(move || {
                            actions.with(|actions| actions.get(index).map(|action| action.label.clone()).unwrap_or_default())
                        });
                        view! {
                            <IconButton
                                glyph
                                label
                                @test_id={format!("{action_id}.action.{index}")}
                                on_click={move || on_action.call(index)}
                            />
                        }
                    }</ForEach>
                </List>
            </List>
        </Frame>
    }
}
