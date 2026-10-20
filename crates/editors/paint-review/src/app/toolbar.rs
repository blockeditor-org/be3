use block_editor_beui::Toolbar;
use block_editor_beui::beui::NodeId;
use block_editor_beui::beui::icons::{
    ICON_CHECK, ICON_CHEVRON_LEFT, ICON_CHEVRON_RIGHT, ICON_COMPARE, ICON_DELETE, ICON_DIFFERENCE,
    ICON_FIT_SCREEN, ICON_PAUSE, ICON_PLAY_ARROW, ICON_REFRESH, ICON_VERTICAL_SPLIT, ICON_ZOOM_IN,
    ICON_ZOOM_OUT,
};
use block_editor_beui::beui::reactive::*;
use block_editor_beui::beui::styled::*;

use super::state::{Review, Showing, Status};

const ZOOM_STEP: f32 = 1.25;

#[component]
pub(crate) fn ReviewToolbar(review: Review, shown: Prop<bool>) -> NodeId {
    let status = create_memo(move || {
        let path = review.selected.get()?;
        review.status(&path)
    });
    let downloading = review.downloading;
    let busy = create_memo(move || downloading.get());
    let approvable = create_memo(move || {
        review.editable()
            && review.pending.get().is_none()
            && matches!(status.get(), Some(Status::New | Status::Modified))
    });
    let unapprovable = create_memo(move || {
        review.editable()
            && matches!(
                status.get(),
                Some(Status::Modified | Status::Removed | Status::Unchanged)
            )
    });
    let approve_off = create_memo(move || !approvable.get());
    let unapprove_off = create_memo(move || !unapprovable.get());
    let comparable = create_memo(move || status.get() == Some(Status::Modified));

    let error = review.error;
    let failed = create_memo(move || error.get().is_some());
    let reason = create_memo(move || error.get().unwrap_or_default());

    view! {
        <Toolbar shown={shown}>
            <IconButton
                glyph={ICON_REFRESH.to_owned()}
                label="Look at the branch again"
                disabled={busy}
                @test_id={"paint_review.refresh"}
                on_click={move || review.refresh()}
            />
            <Button
                label="Approve"
                glyph={ICON_CHECK.to_owned()}
                variant=ButtonVariant::Primary
                disabled={approve_off}
                @test_id={"paint_review.approve"}
                on_click={move || {
                    if let Some(path) = review.selected.get_untracked() {
                        review.approve(&path);
                    }
                }}
            />
            <Button
                label="Unapprove"
                glyph={ICON_DELETE.to_owned()}
                variant=ButtonVariant::Secondary
                disabled={unapprove_off}
                @test_id={"paint_review.unapprove"}
                on_click={move || {
                    if let Some(path) = review.selected.get_untracked() {
                        review.unapprove(&path);
                    }
                }}
            />
            <Show condition={comparable}>
                <ViewChoice review={review} />
            </Show>
            <ZoomControls review={review} />
            <Show condition={failed}>
                <Text style=TextStyle::CAPTION string={reason} color=TextColor::Danger />
            </Show>
        </Toolbar>
    }
}

#[component]
fn ViewChoice(review: Review) -> NodeId {
    let showing = review.showing;
    let pressed = |wanted: Showing| create_memo(move || showing.get() == wanted);
    let choose = |review: Review, wanted: Showing| move |_: bool| review.show(wanted);
    view! {
        <List direction=Direction::Horizontal align=Align::Center spacing=4.0>
            <ToggleButton
                label="Approved"
                pressed={pressed(Showing::Approved)}
                @test_id={"paint_review.view.approved"}
                on_change={choose(review, Showing::Approved)}
            />
            <ToggleButton
                label="Current"
                pressed={pressed(Showing::Current)}
                @test_id={"paint_review.view.current"}
                on_change={choose(review, Showing::Current)}
            />
            <ToggleButton
                label="Difference"
                glyph={ICON_COMPARE.to_owned()}
                pressed={pressed(Showing::Difference)}
                @test_id={"paint_review.view.difference"}
                on_change={choose(review, Showing::Difference)}
            />
            <ToggleButton
                label="Side by side"
                glyph={ICON_VERTICAL_SPLIT.to_owned()}
                pressed={pressed(Showing::SideBySide)}
                @test_id={"paint_review.view.side_by_side"}
                on_change={choose(review, Showing::SideBySide)}
            />
        </List>
    }
}

#[component]
fn ZoomControls(review: Review) -> NodeId {
    let scale = review.scale;
    let zoomed = create_memo(move || scale.get().is_some());
    let percent = create_memo(move || format!("{:.0}%", scale.get().unwrap_or(1.0) * 100.0));
    let editor = review.editor();
    view! {
        <Frame visible={zoomed}>
            <List direction=Direction::Horizontal align=Align::Center spacing=4.0>
                <IconButton
                    glyph={ICON_ZOOM_OUT.to_owned()}
                    label="Zoom out"
                    @test_id={"paint_review.zoom.out"}
                    on_click={move || editor.zoom(1.0 / ZOOM_STEP)}
                />
                <Button
                    label={percent}
                    variant=ButtonVariant::Secondary
                    @test_id={"paint_review.zoom.actual"}
                    on_click={move || {
                        let scale = scale.get_untracked().unwrap_or(1.0);
                        editor.zoom(1.0 / scale);
                    }}
                />
                <IconButton
                    glyph={ICON_ZOOM_IN.to_owned()}
                    label="Zoom in"
                    @test_id={"paint_review.zoom.in"}
                    on_click={move || editor.zoom(ZOOM_STEP)}
                />
                <IconButton
                    glyph={ICON_FIT_SCREEN.to_owned()}
                    label="Fit the painting to the panel"
                    @test_id={"paint_review.zoom.fit"}
                    on_click={move || editor.fit()}
                />
            </List>
        </Frame>
    }
}

#[component]
pub(crate) fn ReviewCaption(review: Review) -> NodeId {
    let path = create_memo(move || review.selected.get().unwrap_or_default());
    let chosen = create_memo(move || review.selected.get().is_some());
    let summary = create_memo(move || {
        let Some(path) = review.selected.get() else {
            return String::new();
        };
        let Some(status) = review.status(&path) else {
            return String::new();
        };
        let showing = review.shown_as(status);
        let mut parts = vec![status.label().to_owned(), showing.label()];
        if showing != Showing::Difference
            && let Some((change, _)) = review.changed(&path, status)
        {
            parts.push(change);
        }
        if let Some(description) = review.description.get() {
            parts.push(description);
        }
        parts.join(" · ")
    });
    view! {
        <Frame visible={chosen} padding={Sides::symmetric(12.0, 6.0)}>
            <List direction=Direction::Horizontal align=Align::Center spacing=8.0>
                <Text string={path} @test_id={"paint_review.path"} />
                <Text style=TextStyle::CAPTION string={summary} @test_id={"paint_review.summary"} />
            </List>
        </Frame>
    }
}

#[component]
pub(crate) fn FrameControls(review: Review, count: Memo<usize>) -> NodeId {
    let many = create_memo(move || count.get() > 1);
    let frame = review.frame;
    let playing = review.playing;
    let first = create_memo(move || frame.get() == 0);
    let last = create_memo(move || frame.get() + 1 >= count.get());
    let glyph = create_memo(move || match playing.get() {
        true => ICON_PAUSE.to_owned(),
        false => ICON_PLAY_ARROW.to_owned(),
    });
    let position = create_memo(move || format!("Frame {} of {}", frame.get() + 1, count.get()));
    let at = create_memo(move || frame.get() as f32);

    let changed_frame = create_memo(move || {
        let path = review.selected.get()?;
        let status = review.status(&path)?;
        review
            .changed(&path, status)?
            .1
            .filter(|at| *at < count.get() && *at != frame.get())
    });
    let differs = create_memo(move || changed_frame.get().is_some());

    view! {
        <Frame visible={many} padding={Sides::symmetric(12.0, 6.0)}>
            <List direction=Direction::Horizontal align=Align::Center spacing=6.0>
                <IconButton
                    glyph={ICON_CHEVRON_LEFT.to_owned()}
                    label="The frame before"
                    disabled={first}
                    @test_id={"paint_review.frame.previous"}
                    on_click={move || review.seek(frame.get_untracked().saturating_sub(1))}
                />
                <IconButton
                    glyph={glyph}
                    label="Play the recording"
                    @test_id={"paint_review.frame.play"}
                    on_click={move || review.toggle_playback()}
                />
                <IconButton
                    glyph={ICON_CHEVRON_RIGHT.to_owned()}
                    label="The frame after"
                    disabled={last}
                    @test_id={"paint_review.frame.next"}
                    on_click={move || review.seek(frame.get_untracked() + 1)}
                />
                <Dynamic value={count}>|count: usize|
                    <Slider
                        value={at}
                        max={(count - 1) as f32}
                        label="Frame"
                        @test_id={"paint_review.frame.at"}
                        on_change={move |value: f32| {
                            review.seek(value.round().max(0.0) as usize)
                        }}
                    />
                </Dynamic>
                <Text style=TextStyle::CAPTION string={position} />
                <Show condition={differs}>
                    <Button
                        label="Changed frame"
                        glyph={ICON_DIFFERENCE.to_owned()}
                        variant=ButtonVariant::Secondary
                        @test_id={"paint_review.frame.changed"}
                        on_click={move || {
                            if let Some(frame) = changed_frame.get_untracked() {
                                review.seek(frame);
                            }
                        }}
                    />
                </Show>
            </List>
        </Frame>
    }
}
