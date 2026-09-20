use std::rc::Rc;

use block_editor_plugin::beui::NodeId;
use block_editor_plugin::beui::icons::{
    ICON_DELETE, ICON_DIFFERENCE, ICON_DONE_ALL, ICON_FIBER_NEW,
};
use block_editor_plugin::beui::reactive::{
    Align, Direction, ForEach, Frame, List, Show, clone, component, create_memo, create_selector,
    view,
};
use block_editor_plugin::beui::styled::{Body, Caption, Icon, ListRow, use_theme};

use crate::download::BRANCH;

use super::state::{Review, Status};

#[component]
pub(crate) fn PaintingList(review: Rc<Review>) -> NodeId {
    let listed = Rc::clone(&review);
    let entries = create_memo(clone!(listed -> move || listed.entries()));
    let ready = create_memo(clone!(entries -> move || entries.get().is_some()));
    let waiting = create_memo(clone!(ready -> move || !ready.get()));
    let empty = create_memo(clone!(entries -> move || {
        entries.get().is_some_and(|entries| entries.is_empty())
    }));
    let downloading = review.downloading.clone();
    let searching =
        create_memo(clone!(empty downloading -> move || empty.get() && downloading.get()));
    let barren =
        create_memo(clone!(empty downloading -> move || empty.get() && !downloading.get()));
    let theme = use_theme();
    let opening = theme.text_muted.clone();
    let looking = theme.text_muted.clone();
    let nothing = format!("No paintings on the {BRANCH} branch");

    view! {
        <List spacing=8.0>
            <Show condition={waiting}>
                <Caption content="Opening the review" color={opening} />
            </Show>
            <ForEach keys={Status::ALL.to_vec()}>
                {move |status: Status| {
                    let review = Rc::clone(&review);
                    let entries = entries.clone();
                    view! {
                        <StatusGroup review entries status />
                    }
                }}
            </ForEach>
            <Show condition={searching}>
                <Caption content={format!("Looking at the {BRANCH} branch")} color={looking} />
            </Show>
            <Show condition={barren}>
                <Caption content={nothing} color={theme.text_muted.clone()} />
            </Show>
        </List>
    }
}

#[component]
fn StatusGroup(
    review: Rc<Review>,
    entries: block_editor_plugin::beui::reactive::Memo<Option<Vec<super::state::Entry>>>,
    status: Status,
) -> NodeId {
    let paths = create_memo(clone!(entries -> move || {
        entries
            .get()
            .unwrap_or_default()
            .into_iter()
            .filter(|entry| entry.status == status)
            .map(|entry| entry.path)
            .collect::<Vec<String>>()
    }));
    let any = create_memo(clone!(paths -> move || !paths.get().is_empty()));
    let heading = create_memo(clone!(paths -> move || {
        format!("{} ({})", status.label(), paths.get().len())
    }));
    let selected = review.selected.clone();
    let here = create_selector(clone!(selected -> move || selected.get()));
    let theme = use_theme();

    view! {
        <Frame visible={any}>
            <List spacing=4.0>
                <List direction=Direction::Horizontal align=Align::Center spacing=6.0>
                    <Icon glyph={glyph(status).to_owned()} color={theme.text_muted.clone()} />
                    <Body content={heading} />
                </List>
                <ForEach keys={paths}>
                    {move |path: String| {
                        let review = Rc::clone(&review);
                        let chosen = here.memo(Some(path.clone()));
                        view! {
                            <PaintingRow review path selected={chosen} />
                        }
                    }}
                </ForEach>
            </List>
        </Frame>
    }
}

#[component]
fn PaintingRow(
    review: Rc<Review>,
    path: String,
    selected: block_editor_plugin::beui::reactive::Memo<bool>,
) -> NodeId {
    let test_id = format!("paint_review.entry.{path}");
    let label = path.clone();
    let chosen = path.clone();
    view! {
        <ListRow
            selected={selected}
            @test_id={test_id}
            on_click={move || review.select(&chosen)}
            on_activate={move || {}}
        >
            <Body content={label} />
        </ListRow>
    }
}

fn glyph(status: Status) -> &'static str {
    match status {
        Status::New => ICON_FIBER_NEW,
        Status::Modified => ICON_DIFFERENCE,
        Status::Removed => ICON_DELETE,
        Status::Unchanged => ICON_DONE_ALL,
    }
}
