use block_editor_beui::beui::NodeId;
use block_editor_beui::beui::icons::{ICON_DELETE, ICON_DIFFERENCE, ICON_DONE_ALL, ICON_FIBER_NEW};
use block_editor_beui::beui::reactive::*;
use block_editor_beui::beui::styled::*;

use crate::download::BRANCH;

use super::state::{Review, Status};
use block_editor_beui::beui::TextAlign;

#[component]
pub(crate) fn PaintingList(review: Review) -> NodeId {
    let entries = create_memo(move || review.entries());
    let ready = create_memo(move || entries.get().is_some());
    let waiting = create_memo(move || !ready.get());
    let empty = create_memo(move || entries.get().is_some_and(|entries| entries.is_empty()));
    let downloading = review.downloading;
    let searching = create_memo(move || empty.get() && downloading.get());
    let barren = create_memo(move || empty.get() && !downloading.get());
    let opening = TextColor::Muted;
    let looking = TextColor::Muted;
    let nothing = format!("No paintings on the {BRANCH} branch");

    view! {
        <List spacing=8.0>
            <Show condition={waiting}>
                <Text style=TextStyle::CAPTION string="Opening the review" color={opening} />
            </Show>
            <ForEach keys={Status::ALL.to_vec()}>|status: Status| {
                let entries = entries;
                view! {
                    <StatusGroup review entries status />
                }
            }</ForEach>
            <Show condition={searching}>
                <Text
                    style=TextStyle::CAPTION
                    string={format!("Looking at the {BRANCH} branch")}
                    color={looking}
                />
            </Show>
            <Show condition={barren}>
                <Text style=TextStyle::CAPTION string={nothing.clone()} />
            </Show>
        </List>
    }
}

#[component]
fn StatusGroup(
    review: Review,
    entries: block_editor_beui::beui::reactive::Memo<Option<Vec<super::state::Entry>>>,
    status: Status,
) -> NodeId {
    let paths = create_memo(move || {
        entries
            .get()
            .unwrap_or_default()
            .into_iter()
            .filter(|entry| entry.status == status)
            .map(|entry| entry.path)
            .collect::<Vec<String>>()
    });
    let any = create_memo(move || !paths.get().is_empty());
    let heading = create_memo(move || format!("{} ({})", status.label(), paths.get().len()));
    let selected = review.selected;
    let here = create_selector(move || selected.get());

    view! {
        <Frame visible={any}>
            <List spacing=4.0>
                <List direction=Direction::Horizontal align=Align::Center spacing=6.0>
                    <Text color=TextColor::Muted align=TextAlign::Center>
                        <Icon glyph={glyph(status).to_owned()} />
                    </Text>
                    <Text string={heading} />
                </List>
                <ForEach keys={paths}>|path: String| {
                    let chosen = here.memo(Some(path.clone()));
                    view! {
                        <PaintingRow review path selected={chosen} />
                    }
                }</ForEach>
            </List>
        </Frame>
    }
}

#[component]
fn PaintingRow(
    review: Review,
    path: String,
    selected: block_editor_beui::beui::reactive::Memo<bool>,
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
            <Text string={label} />
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
