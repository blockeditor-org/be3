use crate::actions::Chord;
use crate::reactive::{Callback, Child, ComponentRef, NodeId, Prop};
use crate::types::{Color, Key, Rect};
use beui_macros::component;

pub const COPY_KEY: Chord = Chord::ctrl(Key::C);
pub const SELECT_ALL_KEY: Chord = Chord::ctrl(Key::A);

#[derive(Clone, Copy, Default)]
pub struct SelectableState(pub(crate) crate::core::selectable::State);

impl SelectableState {
    pub fn selected_text(&self) -> String {
        self.0.selected_text()
    }

    pub fn select_all(&self) {
        self.0.select_all();
    }
}

pub fn copy_selection(state: SelectableState) {
    crate::core::selectable::copy_selection(state.0);
}

pub fn select_all(state: SelectableState) {
    crate::core::selectable::select_all(state.0);
}

#[component]
pub fn Selectable(
    #[prop(default = ComponentRef::new())] api: ComponentRef<SelectableState>,
    children: Child,
    #[prop(default = SelectableState::default())] state: SelectableState,
    color: Prop<Color>,
    handle_color: Prop<Color>,
    caret_width: Prop<f32>,
    #[prop(default = Some(COPY_KEY))] copy_key: Prop<Option<Chord>>,
    #[prop(default = Some(SELECT_ALL_KEY))] select_all_key: Prop<Option<Chord>>,
    on_toolbar: Callback<Option<Rect>>,
) -> NodeId {
    crate::core::selectable::selectable(
        api,
        children,
        state,
        color,
        handle_color,
        caret_width,
        copy_key,
        select_all_key,
        on_toolbar,
    )
}
