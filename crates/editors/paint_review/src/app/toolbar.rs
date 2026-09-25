use std::rc::Rc;

use block_editor_beui::Toolbar;
use block_editor_beui::beui::NodeId;
use block_editor_beui::beui::icons::{
    ICON_CHECK, ICON_CHEVRON_LEFT, ICON_CHEVRON_RIGHT, ICON_COMPARE, ICON_DELETE, ICON_DIFFERENCE,
    ICON_FIT_SCREEN, ICON_PAUSE, ICON_PLAY_ARROW, ICON_REFRESH, ICON_VERTICAL_SPLIT, ICON_ZOOM_IN,
    ICON_ZOOM_OUT,
};
use block_editor_beui::beui::reactive::{
    Align, Direction, Dynamic, Frame, List, Memo, Prop, Show, clone, component, create_memo, view,
};
use block_editor_beui::beui::styled::{
    Body, Button, ButtonVariant, Caption, IconButton, Slider, ToggleButton, use_theme,
};

use super::state::{Review, Showing, Status};

const ZOOM_STEP: f32 = 1.25;

#[component]
pub(crate) fn ReviewToolbar(review: Rc<Review>, shown: Prop<bool>) -> NodeId {
    let bar = Rc::clone(&review);
    let refresh = Rc::clone(&review);
    let approve = Rc::clone(&review);
    let unapprove = Rc::clone(&review);
    let theme = use_theme();

    let status = create_memo(clone!(bar -> move || {
        let path = bar.selected.get()?;
        bar.status(&path)
    }));
    let downloading = review.downloading.clone();
    let busy = create_memo(clone!(downloading -> move || downloading.get()));
    let editable = Rc::clone(&review);
    let approvable = create_memo(clone!(status editable -> move || {
        editable.editable()
            && editable.pending.get().is_none()
            && matches!(status.get(), Some(Status::New | Status::Modified))
    }));
    let unapprovable = create_memo(clone!(status editable -> move || {
        editable.editable()
            && matches!(
                status.get(),
                Some(Status::Modified | Status::Removed | Status::Unchanged)
            )
    }));
    let approve_off = create_memo(clone!(approvable -> move || !approvable.get()));
    let unapprove_off = create_memo(clone!(unapprovable -> move || !unapprovable.get()));
    let comparable = create_memo(clone!(status -> move || status.get() == Some(Status::Modified)));

    let choices = Rc::clone(&review);
    let zooms = Rc::clone(&review);
    let error = review.error.clone();
    let failed = create_memo(clone!(error -> move || error.get().is_some()));
    let reason = create_memo(clone!(error -> move || error.get().unwrap_or_default()));

    view! {
        <Toolbar shown={shown}>
            <IconButton
                glyph={ICON_REFRESH.to_owned()}
                label="Look at the branch again"
                disabled={busy}
                @test_id={"paint_review.refresh"}
                on_click={move || refresh.refresh()}
            />
            <Button
                label="Approve"
                glyph={ICON_CHECK.to_owned()}
                variant=ButtonVariant::Primary
                disabled={approve_off}
                @test_id={"paint_review.approve"}
                on_click={move || {
                    if let Some(path) = approve.selected.get_untracked() {
                        approve.approve(&path);
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
                    if let Some(path) = unapprove.selected.get_untracked() {
                        unapprove.unapprove(&path);
                    }
                }}
            />
            <Show condition={comparable}>
                <ViewChoice review={choices} />
            </Show>
            <ZoomControls review={zooms} />
            <Show condition={failed}>
                <Caption content={reason} color={theme.danger.clone()} />
            </Show>
        </Toolbar>
    }
}

#[component]
fn ViewChoice(review: Rc<Review>) -> NodeId {
    let showing = review.showing.clone();
    let pressed = |wanted: Showing| create_memo(clone!(showing -> move || showing.get() == wanted));
    let choose = |review: &Rc<Review>, wanted: Showing| {
        let review = Rc::clone(review);
        move |_: bool| review.show(wanted)
    };
    view! {
        <List direction=Direction::Horizontal align=Align::Center spacing=4.0>
            <ToggleButton
                label="Approved"
                pressed={pressed(Showing::Approved)}
                @test_id={"paint_review.view.approved"}
                on_change={choose(&review, Showing::Approved)}
            />
            <ToggleButton
                label="Current"
                pressed={pressed(Showing::Current)}
                @test_id={"paint_review.view.current"}
                on_change={choose(&review, Showing::Current)}
            />
            <ToggleButton
                label="Difference"
                glyph={ICON_COMPARE.to_owned()}
                pressed={pressed(Showing::Difference)}
                @test_id={"paint_review.view.difference"}
                on_change={choose(&review, Showing::Difference)}
            />
            <ToggleButton
                label="Side by side"
                glyph={ICON_VERTICAL_SPLIT.to_owned()}
                pressed={pressed(Showing::SideBySide)}
                @test_id={"paint_review.view.side_by_side"}
                on_change={choose(&review, Showing::SideBySide)}
            />
        </List>
    }
}

#[component]
fn ZoomControls(review: Rc<Review>) -> NodeId {
    let scale = review.scale.clone();
    let zoomed = create_memo(clone!(scale -> move || scale.get().is_some()));
    let percent = create_memo(clone!(scale -> move || {
        format!("{:.0}%", scale.get().unwrap_or(1.0) * 100.0)
    }));
    let out = review.editor().clone();
    let inward = review.editor().clone();
    let fit = review.editor().clone();
    let actual = review.editor().clone();
    let actual_scale = scale.clone();
    view! {
        <Frame visible={zoomed}>
            <List direction=Direction::Horizontal align=Align::Center spacing=4.0>
                <IconButton
                    glyph={ICON_ZOOM_OUT.to_owned()}
                    label="Zoom out"
                    @test_id={"paint_review.zoom.out"}
                    on_click={move || out.zoom(1.0 / ZOOM_STEP)}
                />
                <Button
                    label={percent}
                    variant=ButtonVariant::Secondary
                    @test_id={"paint_review.zoom.actual"}
                    on_click={move || {
                        let scale = actual_scale.get_untracked().unwrap_or(1.0);
                        actual.zoom(1.0 / scale);
                    }}
                />
                <IconButton
                    glyph={ICON_ZOOM_IN.to_owned()}
                    label="Zoom in"
                    @test_id={"paint_review.zoom.in"}
                    on_click={move || inward.zoom(ZOOM_STEP)}
                />
                <IconButton
                    glyph={ICON_FIT_SCREEN.to_owned()}
                    label="Fit the painting to the panel"
                    @test_id={"paint_review.zoom.fit"}
                    on_click={move || fit.fit()}
                />
            </List>
        </Frame>
    }
}

#[component]
pub(crate) fn ReviewCaption(review: Rc<Review>) -> NodeId {
    let named = Rc::clone(&review);
    let path = create_memo(clone!(named -> move || named.selected.get().unwrap_or_default()));
    let chosen = create_memo(clone!(named -> move || named.selected.get().is_some()));
    let described = Rc::clone(&review);
    let summary = create_memo(clone!(described -> move || {
        let Some(path) = described.selected.get() else {
            return String::new();
        };
        let Some(status) = described.status(&path) else {
            return String::new();
        };
        let showing = described.shown_as(status);
        let mut parts = vec![status.label().to_owned(), showing.label()];
        if showing != Showing::Difference
            && let Some((change, _)) = described.changed(&path, status)
        {
            parts.push(change);
        }
        if let Some(description) = described.description.get() {
            parts.push(description);
        }
        parts.join(" · ")
    }));
    let theme = use_theme();
    view! {
        <Frame visible={chosen} padding_horizontal=12.0 padding_vertical=6.0>
            <List direction=Direction::Horizontal align=Align::Center spacing=8.0>
                <Body content={path} @test_id={"paint_review.path"} />
                <Caption
                    content={summary}
                    color={theme.text_muted.clone()}
                    @test_id={"paint_review.summary"}
                />
            </List>
        </Frame>
    }
}

#[component]
pub(crate) fn FrameControls(review: Rc<Review>, count: Memo<usize>) -> NodeId {
    let many = create_memo(clone!(count -> move || count.get() > 1));
    let frame = review.frame.clone();
    let playing = review.playing.clone();
    let first = create_memo(clone!(frame -> move || frame.get() == 0));
    let last = create_memo(clone!(frame count -> move || frame.get() + 1 >= count.get()));
    let glyph = create_memo(clone!(playing -> move || match playing.get() {
        true => ICON_PAUSE.to_owned(),
        false => ICON_PLAY_ARROW.to_owned(),
    }));
    let position = create_memo(clone!(frame count -> move || {
        format!("Frame {} of {}", frame.get() + 1, count.get())
    }));
    let at = create_memo(clone!(frame -> move || frame.get() as f32));

    let back = Rc::clone(&review);
    let back_frame = frame.clone();
    let forward = Rc::clone(&review);
    let forward_frame = frame.clone();
    let play = Rc::clone(&review);
    let slid = Rc::clone(&review);

    let changed = Rc::clone(&review);
    let changed_frame = create_memo(clone!(changed count frame -> move || {
        let path = changed.selected.get()?;
        let status = changed.status(&path)?;
        changed
            .changed(&path, status)?
            .1
            .filter(|at| *at < count.get() && *at != frame.get())
    }));
    let differs = create_memo(clone!(changed_frame -> move || changed_frame.get().is_some()));
    let jump = Rc::clone(&review);
    let jump_frame = changed_frame.clone();
    let theme = use_theme();

    view! {
        <Frame visible={many} padding_horizontal=12.0 padding_vertical=6.0>
            <List direction=Direction::Horizontal align=Align::Center spacing=6.0>
                <IconButton
                    glyph={ICON_CHEVRON_LEFT.to_owned()}
                    label="The frame before"
                    disabled={first}
                    @test_id={"paint_review.frame.previous"}
                    on_click={move || back.seek(back_frame.get_untracked().saturating_sub(1))}
                />
                <IconButton
                    glyph={glyph}
                    label="Play the recording"
                    @test_id={"paint_review.frame.play"}
                    on_click={move || play.toggle_playback()}
                />
                <IconButton
                    glyph={ICON_CHEVRON_RIGHT.to_owned()}
                    label="The frame after"
                    disabled={last}
                    @test_id={"paint_review.frame.next"}
                    on_click={move || forward.seek(forward_frame.get_untracked() + 1)}
                />
                <Dynamic value={count.clone()}>
                    {move |count: usize| {
                        let slid = Rc::clone(&slid);
                        let at = at.clone();
                        view! {
                            <Slider
                                value={at}
                                max={(count - 1) as f32}
                                label="Frame"
                                @test_id={"paint_review.frame.at"}
                                on_change={move |value: f32| {
                                    slid.seek(value.round().max(0.0) as usize)
                                }}
                            />
                        }
                    }}
                </Dynamic>
                <Caption content={position} color={theme.text_muted.clone()} />
                <Show condition={differs}>
                    <Button
                        label="Changed frame"
                        glyph={ICON_DIFFERENCE.to_owned()}
                        variant=ButtonVariant::Secondary
                        @test_id={"paint_review.frame.changed"}
                        on_click={move || {
                            if let Some(frame) = jump_frame.get_untracked() {
                                jump.seek(frame);
                            }
                        }}
                    />
                </Show>
            </List>
        </Frame>
    }
}
