use beui::NodeId;
use beui::reactive::*;
use beui::styled::*;

use super::state::State;

#[component]
pub(crate) fn ImportError(state: State) -> NodeId {
    view! {
        <Banner
            message={state.import_error}
            id="text.import-error"
            on_dismiss={move || state.import_error.set(None)}
        />
    }
}
