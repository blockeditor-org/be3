use super::*;
use crate::EditorBlock;
use be_block::BlockContent as _;
use be_sdk_api::Input;
use be_sdk_api::embed::blocks::{self as wire, Call};
use be_sdk_api::ops::OpId;
use be_sdk_type_guest::editor::{FromEditor, ToEditor};
use beui::Pos2;

const INSTANCE: EditorInstanceId = EditorInstanceId(1);
const REGION: ViewId = ViewId::MAIN;
const PASS: u64 = 1;
const SIZE: Vec2 = vec2(100.0, 100.0);

fn tell(instances: &mut Instances, message: ShellMessage) -> bool {
    instances.shell_call(ShellCall::Tell {
        reply: reply(next_op()),
        instance: INSTANCE,
        messages: vec![message],
    })
}

/// What instances report from now on, heard once the frame's mail is delivered
/// (`host::deliver`).
fn hearing() -> (
    host::Mailbox,
    std::rc::Rc<std::cell::RefCell<Vec<InstanceEvent>>>,
) {
    let heard = std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
    let into = std::rc::Rc::clone(&heard);
    let (mailbox, post) =
        host::mailbox(move |heard: crate::Heard| into.borrow_mut().push(heard.event));
    crate::events::listen(None, post);
    (mailbox, heard)
}

fn placed() -> Instances {
    placed_on(Uuid::nil(), Uuid::nil())
}

fn placed_on(block: Uuid, block_type: Uuid) -> Instances {
    let rect = Rect::from_min_size(pos2(10.0, 10.0), SIZE);
    let mut instances = Instances::default();
    let block_types = Arc::new(crate::AppCatalog::default());
    let role = InstanceRole::Editor(
        Uuid::nil(),
        EditorBlock {
            id: block,
            block_type,
            view_block: None,
        },
    );
    instances.report(
        INSTANCE,
        View::MAIN,
        Uuid::nil(),
        role,
        &block_types,
        Some(be_sdk_api::FrameSpec::default()),
        SIZE,
        Rect::from_min_size(Pos2::ZERO, SIZE),
        1.0,
        PASS,
    );
    instances.place(INSTANCE, REGION, Placement { rect, clip: rect });
    instances
}

fn forwarded(
    events: Vec<beui::Forwarded>,
    pointer: Option<Pos2>,
    focused: bool,
) -> beui::ForwardedInput {
    let rect = Rect::from_min_size(pos2(10.0, 10.0), SIZE);
    beui::ForwardedInput {
        events,
        rect,
        hovered: pointer.is_some_and(|pointer| rect.contains(pointer)),
        focused,
        pointer,
        modifiers: beui::Modifiers::NONE,
    }
}

fn moved(pos: Pos2) -> beui::Forwarded {
    beui::Forwarded::Pointer {
        event: beui::PointerEvent {
            pointer: beui::PointerId::Mouse,
            window: beui::WindowId::MAIN,
            phase: beui::PointerPhase::Move,
            pos,
            modifiers: beui::Modifiers::NONE,
            clicks: 0,
            force: None,
            pen: None,
            predicted: None,
            time: std::time::Instant::now(),
            source: 0,
            rested: false,
        },
        over: None,
    }
}

fn input_events<T: Clone + Into<Outbound>>(messages: &[T]) -> Vec<Input> {
    messages
        .iter()
        .cloned()
        .map(Into::into)
        .filter_map(|message| match message {
            Outbound::App(AppCall::Input(batch)) => {
                Some(batch.events.into_iter().map(|event| event.input))
            }
            _ => None,
        })
        .flatten()
        .collect()
}

fn publish<W: be_sdk_api::Watched>(instances: &mut Instances, value: &W::Value) -> bool {
    instances.publish(W::OPEN, Arc::new(be_sdk_api::ops::encode(value)))
}

fn watched_sent<W: be_sdk_api::Watched>(messages: &[Outbound]) -> Vec<W::Value> {
    messages
        .iter()
        .filter_map(|message| match message {
            Outbound::Editor(instance, EditorCall::Watched { open, value })
                if *instance == INSTANCE && *open == W::OPEN =>
            {
                be_sdk_api::ops::decode(value)
            }
            _ => None,
        })
        .collect()
}

fn watch<W: be_sdk_api::Watched>(instances: &mut Instances) -> bool {
    instances.watch_published(INSTANCE, W::OPEN)
}

fn host_call<W: be_sdk_api::Watched, A: serde::Serialize>(
    instances: &mut Instances,
    function: &str,
    args: &A,
) -> bool {
    instances.published_call(
        INSTANCE,
        be_sdk_api::ops::Started {
            op: next_op(),
            parent: None,
            method: be_sdk_api::ops::method_id(&format!("{}#{function}", W::INTERFACE)),
            args: be_sdk_api::ops::encode(args),
        },
    )
}

fn configure(instances: &mut Instances) -> Option<AppCall> {
    let next = instances.next_screens(PASS);
    let mut messages = next.opened;
    messages.push(AppCall::Screens(instances.screen_set(next.screens)).into());
    instances.configure(&messages)
}

fn child(rect: Rect, follows_view: bool) -> ChildPlacements {
    ChildPlacements {
        instance: INSTANCE,
        view: REGION,
        children: vec![ChildPlacement {
            child: ChildId(1),
            content: ChildContent::Block {
                block_id: [1; 16],
                block_type: [0; 16],
                view_block: None,
                editor: None,
            },
            rect,
            clip: Rect::from_min_size(pos2(-1000.0, -1000.0), vec2(2000.0, 2000.0)),
            own_frame: false,
            top_bar: be_sdk_api::TopBar::Hidden,
            corner_radius: 0.0,
            layer: be_sdk_api::ChildLayer::Below,
            mode: ChildMode::Passive,
            intrinsic: None,
            fit: None,
            rotation: 0.0,
            opacity: 1.0,
            follows_view,
            dialog: None,
            docks: None,
            anchor: None,
            fullscreen: false,
        }],
        occluders: Vec::new(),
    }
}

fn next_screens(instances: &mut Instances) -> NextScreens {
    instances.touch(&be_host_stack::take_touched());
    instances.next_screens(PASS)
}

static NEXT_OP: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);

fn next_op() -> OpId {
    NEXT_OP.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
}

thread_local! {
    static HOST: be_sdk_api::ops::Connection =
        be_sdk_api::ops::Connection::new(be_sdk_api::ops::Side::Host);
}

fn reply<T, S>(op: OpId) -> be_sdk_api::ops::Reply<T, S> {
    HOST.with(|host| be_sdk_api::ops::Reply::new(host, op))
}

fn call(instances: &mut Instances, call: Call) -> bool {
    instances.blocks_call(INSTANCE, call)
}

fn hold(instances: &mut Instances, block: Uuid, content_type: Uuid) -> OpId {
    let op = next_op();
    assert!(call(
        instances,
        Call::ReplicaNew {
            resource: wire::Replica(op),
            block: block.into_bytes(),
            content_type: content_type.into_bytes(),
        }
    ));
    op
}

fn open_watch(instances: &mut Instances, open: impl FnOnce(OpId) -> Call) -> OpId {
    let op = next_op();
    assert!(call(instances, open(op)));
    op
}

fn answered<T: be_sdk_api::ops::DeserializeOwned>(
    instances: &mut Instances,
    watch: OpId,
) -> Option<T> {
    let (_, bytes) = instances.answers(&[watch]).pop()?;
    Some(be_sdk_api::ops::decode(&bytes).expect("an answer decodes"))
}

fn replica_of(instances: &mut Instances, block: Uuid) -> Option<OpId> {
    let entry = instances.entries.get(&INSTANCE)?;
    if let Some(op) = entry.replicas.get(&block) {
        return Some(*op);
    }
    let own = entry.role.block().filter(|own| own.id == block)?;
    let content_type = entry
        .content
        .as_ref()
        .map_or(own.block_type, |link| link.content_type);
    Some(hold(instances, block, content_type))
}

fn to_replica(instances: &mut Instances, block: Uuid, message: &FromEditor) -> bool {
    let Some(replica) = replica_of(instances, block) else {
        return false;
    };
    call(
        instances,
        Call::ReplicaSend {
            this: wire::Replica(replica),
            frame: be_sdk_type_api::encode_frame(message),
            reply: reply(next_op()),
        },
    )
}

fn from_replica(instances: &mut Instances, block: Uuid) -> Vec<ToEditor> {
    let replica = instances
        .entries
        .get(&INSTANCE)
        .and_then(|entry| entry.replicas.get(&block).copied());
    let mut frames = Vec::new();
    instances.relay(|op, frame| {
        if Some(op) == replica {
            frames.extend(be_host_stack::unframe(frame).and_then(|bytes| ToEditor::decode(&bytes)));
        }
        true
    });
    frames
}

fn replica_sends(instances: &mut Instances, block: Uuid) -> Vec<ToEditor> {
    next_screens(instances);
    from_replica(instances, block)
}

fn replica_sent_until(
    instances: &mut Instances,
    block: Uuid,
    done: impl Fn(&[ToEditor]) -> bool,
) -> Vec<ToEditor> {
    let mut sent = Vec::new();
    loop {
        sent.extend(replica_sends(instances, block));
        if done(&sent) {
            return sent;
        }
        be_host_stack::wait_until(|shared| {
            shared.editor_frames.contains_key(&INSTANCE.0).then_some(())
        });
    }
}

fn window() -> Rect {
    Rect::from_min_max(Pos2::ZERO, pos2(400.0, 300.0))
}

fn place_layered(instances: &mut Instances, origin: Pos2, bounds: Rect) {
    let rect = Rect::from_min_size(origin, SIZE);
    instances.place_mounted(
        INSTANCE,
        REGION,
        Some(be_sdk_api::FrameSpec::default()),
        SIZE,
        Rect::from_min_size(Pos2::ZERO, SIZE),
        1.0,
        1.0,
        false,
        None,
        None,
        Some(be_sdk_api::AnchorSpace {
            to_host: be_sdk_api::HostTransform {
                origin,
                scale: vec2(1.0, 1.0),
            },
            bounds,
        }),
        false,
    );
    instances.place(INSTANCE, REGION, Placement { rect, clip: rect });
}

fn layered(origin: Pos2) -> Instances {
    let mut instances = placed();
    place_layered(&mut instances, origin, window());
    lay_out(&mut instances);
    instances
}

fn lay_out(instances: &mut Instances) {
    configure(instances);
    let serial = instances.serial();
    instances.confirm(serial, &HashSet::default());
}

struct Layer {
    id: u64,
    placement: be_sdk_api::AnchoredPlacement,
}

fn layer(id: u64, parent: u64, rect: Rect) -> Layer {
    Layer {
        id,
        placement: be_sdk_api::AnchoredPlacement {
            parent: ViewId(parent),
            anchor: be_sdk_api::ViewAnchor::Rect(Rect::from_min_size(Pos2::ZERO, vec2(10.0, 10.0))),
            rect,
            dismiss_on_outside_press: true,
            trigger: None,
            keyboard: false,
            blocks_owner: false,
        },
    }
}

fn anchored_screen(id: u64) -> ScreenId {
    ScreenId::opened(id)
}

fn report_anchored(instances: &mut Instances, layers: Vec<Layer>) {
    let layout = be_sdk_api::ScreenLayout {
        screens: layers
            .iter()
            .map(|layer| be_sdk_api::ScreenPlacement {
                screen: anchored_screen(layer.id),
                instance: INSTANCE,
                view: ViewId(layer.id),
                surface: 6 + layer.id as u32,
                width: 100,
                height: 100,
                scale_factor_millis: 1000,
                raster_millis: 1000,
            })
            .collect(),
    };
    let reports: Vec<be_sdk_api::FrameReport> = layers
        .iter()
        .map(|layer| {
            let own = Rect::from_min_size(Pos2::ZERO, layer.placement.rect.size());
            be_sdk_api::FrameReport {
                screen: anchored_screen(layer.id),
                content: None,
                painted: vec![own],
                floating: vec![own],
                floating_painted: Vec::new(),
                marks: Vec::new(),
                marks_painted: Vec::new(),
                scrims: Vec::new(),
                drawing: Vec::new(),
                claims: Vec::new(),
                description: None,
                actions: None,
                anchored: Some(layer.placement),
                work_area: None,
                backdrop: None,
            }
        })
        .collect();
    instances.keep_anchored(&layout);
    instances.adopt_anchored(&reports, &layout);
    instances.set_frame_reports(reports);
}

fn shown_rects(instances: &Instances) -> Vec<Rect> {
    instances
        .shown_anchored(INSTANCE, REGION)
        .into_iter()
        .map(|layer| layer.rect)
        .collect()
}

/// The view of a child of another app the instance's gift of the keyboard goes to.
fn notes_view() -> gift::GiftView {
    ("be3.text".to_owned(), EditorInstanceId(10), ViewId::MAIN)
}

/// The instance's own main view, which holds the keys typed into it while its gift is pending.
fn own_view() -> gift::GiftView {
    (String::new(), INSTANCE, REGION)
}

fn letter(text: &str) -> beui::Forwarded {
    beui::Forwarded::Input(beui::Event::Key {
        press: beui::KeyPress {
            key: beui::Key::A,
            pressed: true,
            repeat: false,
            modifiers: beui::Modifiers::NONE,
        },
        evdev: None,
        text: Some(text.to_owned()),
    })
}

fn press() -> beui::Forwarded {
    let beui::Forwarded::Pointer { mut event, over } = moved(pos2(20.0, 20.0)) else {
        unreachable!("moved forwards a pointer event")
    };
    event.phase = beui::PointerPhase::Down(beui::PointerButton::Primary);
    beui::Forwarded::Pointer { event, over }
}

/// Where an app stands: frames asked and answered, whether its content arrived, whether a text
/// field in the view has the keyboard.
fn progress(frames: (u64, u64), content: bool, typing: bool) -> gift::KeyProgress {
    gift::KeyProgress {
        frames,
        content,
        typing,
    }
}

/// Routes keys typed into the instance's own view, as its holder, in host pass `pass`.
fn own_keys(
    instances: &mut Instances,
    app: gift::KeyProgress,
    focused: bool,
    events: Vec<beui::Forwarded>,
    pass: u64,
) -> Vec<beui::Forwarded> {
    let mut input = forwarded(events, None, focused);
    instances.hold_keys(INSTANCE, &own_view(), app, &mut input, pass);
    input.events
}

/// Routes keys typed into the child's view the instance gave the keyboard to, in host pass `pass`.
fn notes_keys(
    instances: &mut Instances,
    app: gift::KeyProgress,
    focused: bool,
    events: Vec<beui::Forwarded>,
    pass: u64,
) -> Vec<beui::Forwarded> {
    let mut input = forwarded(events, None, focused);
    instances.owe_keys(INSTANCE, &notes_view(), app, &mut input, pass);
    input.events
}

fn resize(instances: &mut Instances, size: Vec2, scale_factor: f32) {
    let role = InstanceRole::Editor(
        Uuid::nil(),
        EditorBlock {
            id: Uuid::nil(),
            block_type: Uuid::nil(),
            view_block: None,
        },
    );
    instances.report(
        INSTANCE,
        View::MAIN,
        Uuid::nil(),
        role,
        &Arc::new(crate::AppCatalog::default()),
        Some(be_sdk_api::FrameSpec::default()),
        size,
        Rect::from_min_size(Pos2::ZERO, size),
        scale_factor,
        PASS,
    );
}

mod a_block_an_app_makes_names_the_app_and_the_app_cannot_say_otherwise;
mod a_block_gift_becomes_a_gift_to_the_child_that_shows_its_block;
mod a_block_gift_counts_only_its_own_instances_commits;
mod a_block_gift_ends_when_no_child_shows_its_block;
mod a_block_gift_ends_when_the_instance_goes;
mod a_block_gift_for_a_block_the_host_refused_to_make_never_starts;
mod a_block_is_named_after_its_content_until_someone_names_it;
mod a_block_shown_before_its_instance_reports_reaches_it_once_it_does;
mod a_child_asked_full_screen_or_closed_reaches_the_host;
mod a_child_placed_in_an_anchored_view_is_shown_in_host_coordinates_inside_it;
mod a_child_that_never_takes_the_keyboard_ends_the_gift;
mod a_click_outside_a_frame_child_hands_the_frame_back;
mod a_database_view_given_content_references_its_database;
mod a_focused_screen_hears_it_has_the_keyboard_again_after_its_app_restarts;
mod a_frame_takeover_keeps_the_last_painting_where_it_was;
mod a_gift_ends_on_the_frame_clock_when_its_editor_commits_no_layout;
mod a_hidden_window_child_takes_no_room_and_is_kept;
mod a_key_intercepted_for_an_app_reaches_it_unless_it_has_the_keyboard;
mod a_key_intercepted_for_an_app_reaches_it_with_its_modifiers;
mod a_listing_the_host_could_not_load_reaches_the_app_as_failed;
mod a_menu_pick_lets_an_instance_open_the_palette_only_in_the_frames_right_after_it;
mod a_message_waits_for_the_instance_it_names_to_be_opened;
mod a_moved_view_moves_its_anchored_views_with_it_and_a_pan_does_not;
mod a_paste_names_where_a_cut_reference_was_cut_from_once;
mod a_press_on_a_blocking_menus_trigger_closes_it_unless_the_press_reaches_the_trigger;
mod a_press_on_an_anchored_views_trigger_leaves_it_to_the_trigger;
mod a_press_outside_a_dismissable_chain_closes_it_and_what_hangs_from_it;
mod a_preview_keeps_the_menu_of_a_tell_and_drops_its_act;
mod a_render_still_asked_is_answered_with_nothing_when_its_instance_goes;
mod a_resize_leaves_children_where_their_frame_painted_them;
mod a_restarted_app_hears_its_childrens_statuses_again;
mod a_restarted_app_is_sent_its_content_and_blocks_again;
mod a_restarted_app_no_longer_shows_its_views_stopped;
mod a_screen_asked_to_take_the_keyboard_reaches_its_view;
mod a_stale_input_method_does_not_release_owed_keys;
mod a_tap_intercepted_for_an_app_reaches_it_unless_it_has_the_keyboard;
mod a_view_attached_to_an_edge_stays_on_it_when_the_view_it_hangs_from_moves;
mod a_view_not_reported_yet_is_backed_by_what_its_instance_last_reported;
mod a_watched_value_reaches_an_instance_watching_it_again_after_a_restart;
mod a_window_the_app_opened_is_shown_and_asked_for_by_its_own_screen;
mod a_windows_actions_are_offered_while_it_is_shown_and_again_when_shown_again;
mod a_zoomed_view_is_shown_at_the_zoom_its_frame_was_painted_at;
mod after_the_first_snapshot_an_editor_is_sent_operations;
mod an_active_child_takes_the_frame_of_the_dock_view_holding_the_content;
mod an_anchored_view_is_shown_from_the_report_that_opens_it_clipped_to_its_bounds;
mod an_app_hears_every_sample_only_while_it_asks;
mod an_app_is_told_when_the_pointer_leaves_it;
mod an_app_makes_a_blocks_agent_only_if_its_manifest_allows_and_only_for_its_own_block;
mod an_app_makes_an_agent_for_a_block_it_made_only_while_it_sits_alone_in_unfiled;
mod an_app_reaches_only_the_hosts_its_manifest_names;
mod an_app_seeds_only_blocks_it_holds;
mod an_editor_can_read_and_edit_a_block_it_watches;
mod an_editor_is_only_sent_messages_its_app_session_accepts;
mod an_input_capture_opened_again_is_not_told_the_earlier_chord;
mod an_instance_inside_a_checkout_speaks_in_its_local_ids;
mod an_instance_the_app_never_opened_is_not_closed;
mod an_instance_watching_a_blocks_head_is_sent_its_stored_bytes;
mod an_instance_watching_a_blocks_history_is_told_when_it_changes;
mod any_host_is_still_reached_only_over_https;
mod children_laid_out_against_an_older_view_are_moved_to_the_current_one;
mod children_of_a_dock_view_holding_the_content_follow_its_camera;
mod clearing_a_name_names_the_block_after_its_content_at_once;
mod closing_an_editor_calls_off_the_gesture_it_left_open;
mod host_calls_are_taken_by_kind_in_the_order_they_came;
mod input_for_a_screen_the_app_does_not_have_yet_reaches_it_once_announced;
mod input_is_withheld_from_screens_the_app_no_longer_has;
mod input_on_an_anchored_view_reaches_its_own_screen_in_its_own_points;
mod keys_owed_to_a_child_wait_again_when_its_content_is_replaced;
mod keys_owed_to_a_view_that_lost_the_keyboard_before_seeing_it_are_not_replayed;
mod keys_typed_after_a_press_the_app_was_never_sent_are_not_held;
mod keys_typed_before_a_child_takes_the_keyboard_wait_for_it;
mod keys_typed_right_after_a_press_wait_for_the_frame_that_answers_it;
mod modifiers_let_go_in_one_view_are_held_again_in_another;
mod revision_history_editors_cannot_mutate_any_block;
mod the_contacts_reach_only_the_shell;
mod the_latest_block_gift_replaces_the_one_before;
mod the_placer_hears_whom_it_gives_the_keyboard_to;
mod the_program_windows_reach_only_the_shell;
mod the_view_a_screen_is_given_carries_its_scale;
