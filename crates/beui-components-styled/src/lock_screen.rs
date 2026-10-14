use beui_core::layer::LayerLevel;
use std::rc::Rc;

use accesskit::{Node, Role};
use beui_macros::{component, view};

use crate::icon_button::IconButton;
use crate::text::{Body, Caption, Display, Title};
use crate::text_input::TextInput;
use crate::theme::use_theme;
use beui_core::base::overlay::{OverlayAnchor, Placement};
use beui_core::geometry::Rect;
use beui_core::node::NodeId;
use beui_core::screens::bounds;
use beui_view::components::overlay::Overlay;
use beui_view::reactive::{
    Align, Callback, Direction, ForEach, Frame, IntoChild, Layer, Layers, List, Memo, Prop, Show,
    component_accessibility, create_memo, create_signal, use_screens,
};

const FIELD_WIDTH: f32 = 280.0;
const SPACING: f32 = 8.0;
const GROUP_SPACING: f32 = 28.0;
const ACTION_SPACING: f32 = 4.0;

#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub struct LockAction {
    pub label: String,
    pub glyph: String,
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
    let screens = use_screens();
    let bounds = create_memo(move || screens.with(|screens| bounds(screens)));
    let places = create_memo(move || (0..screens.with(Vec::len)).collect::<Vec<usize>>());
    let anchor = create_memo(move || OverlayAnchor::Point(bounds.get().min));
    let width = create_memo(move || Some(bounds.get().width()));
    let height = create_memo(move || Some(bounds.get().height()));
    let (password, set_password) = create_signal(String::new());
    let submit = Rc::new(move |typed: String| {
        if busy.get_untracked() || typed.is_empty() {
            return;
        }
        set_password.set(String::new());
        on_submit.call(typed);
    });
    view! {
        <Overlay
            anchor={anchor}
            open={open}
            placement=Placement::At
            scrim={theme.background}
            level=LayerLevel::Lock
            content={move || view! {
                <Frame width={width} height={height}>
                    <Layers>
                        <ForEach keys={places}>
                            {move |index: usize| {
                                let place = create_memo(move || {
                                    let origin = bounds.get().min.to_vec2();
                                    screens.with(|screens| {
                                        screens
                                            .get(index)
                                            .map_or(Rect::ZERO, |screen| screen.rect.translate(-origin))
                                    })
                                });
                                let focused = create_memo(move || {
                                    index == 0 && open.get() && !busy.get()
                                });
                                let submit = submit.clone();
                                let action = on_action;
                                let changed = set_password;
                                let card = view! {
                                    <LockCard
                                        place={place}
                                        time={time}
                                        date={date}
                                        user={user}
                                        error={error}
                                        busy={busy}
                                        actions={actions}
                                        password={password}
                                        focused={focused}
                                        id={format!("{}.{index}", id)}
                                        on_change={move |typed: String| changed.set(typed)}
                                        on_submit={move |typed: String| submit(typed)}
                                        on_action={action}
                                    />
                                };
                                let layer: Layer = card.into_child();
                                layer
                            }}
                        </ForEach>
                    </Layers>
                </Frame>
            }}
        />
    }
}

#[component]
fn LockCard(
    place: Memo<Rect>,
    time: Memo<String>,
    date: Memo<String>,
    user: Memo<String>,
    error: Memo<Option<String>>,
    busy: Memo<bool>,
    actions: Memo<Vec<LockAction>>,
    password: beui_view::reactive::ReadSignal<String>,
    focused: Memo<bool>,
    id: String,
    on_change: Callback<String>,
    on_submit: Callback<String>,
    on_action: Callback<usize>,
) -> NodeId {
    let theme = use_theme();
    let left = create_memo(move || Some(place.get().left()));
    let top = create_memo(move || Some(place.get().top()));
    let width = create_memo(move || Some(place.get().width()));
    let height = create_memo(move || Some(place.get().height()));
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
    view! {
        <Frame
            padding_left={left}
            padding_top={top}
            align_horizontal=Align::Start
            align_vertical=Align::Start
        >
            <Frame
                width={width}
                height={height}
                align_horizontal=Align::Center
                align_vertical=Align::Center
                @test_id={id.clone()}
            >
                <List spacing=GROUP_SPACING align=Align::Center>
                    <List spacing=SPACING align=Align::Center>
                        <Display content={time} />
                        <Body content={date} color={theme.text_muted} />
                    </List>
                    <List spacing=SPACING align=Align::Center>
                        <Title content={user} />
                        <Frame width=FIELD_WIDTH>
                            <TextInput
                                value={password}
                                placeholder="Password"
                                label="Password"
                                password=true
                                disabled={busy}
                                focused={focused}
                                @test_id={format!("{id}.password")}
                                on_change={on_change}
                                on_submit={on_submit}
                            />
                        </Frame>
                        <Show condition={checking}>
                            <Caption content="Checking…" />
                        </Show>
                        <Show condition={failed}>
                            <Caption
                                content={reason}
                                color={theme.danger}
                                @test_id={error_id.clone()}
                            />
                        </Show>
                    </List>
                    <List direction=Direction::Horizontal spacing=ACTION_SPACING>
                        <ForEach keys={slots}>
                            {move |index: usize| {
                                let chose = on_action;
                                let glyph = create_memo(move || {
                                    actions.with(|actions| actions.get(index).map(|action| action.glyph.clone()).unwrap_or_default())
                                });
                                let label = create_memo(move || {
                                    actions.with(|actions| actions.get(index).map(|action| action.label.clone()).unwrap_or_default())
                                });
                                view! {
                                    <IconButton
                                        glyph
                                        label
                                        @test_id={format!("{action_id}.action.{index}")}
                                        on_click={move || chose.call(index)}
                                    />
                                }
                            }}
                        </ForEach>
                    </List>
                </List>
            </Frame>
        </Frame>
    }
}
