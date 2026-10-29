use super::*;
use crate::EditorBlock;
use be_block::BlockContent as _;
use beui::Pos2;
use block_plugin_api::Input;
use block_plugin_api::embed::blocks::{self as wire, Call};
use block_plugin_api::ops::OpId;
use block_type_guest::editor::{FromEditor, ToEditor};

const INSTANCE: EditorInstanceId = EditorInstanceId(1);
const REGION: EditorRegion = EditorRegion::Frame;
const PASS: u64 = 1;
const SIZE: Vec2 = vec2(100.0, 100.0);

fn placed() -> Instances {
    placed_on(Uuid::nil(), Uuid::nil())
}

fn placed_on(block: Uuid, block_type: Uuid) -> Instances {
    let rect = Rect::from_min_size(pos2(10.0, 10.0), SIZE);
    let mut instances = Instances::default();
    let block_types = Arc::new(crate::PluginCatalog::default());
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
        REGION,
        Uuid::nil(),
        role,
        &block_types,
        Some(block_plugin_api::FrameSpec::default()),
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
            Outbound::Plugin(PluginCall::Input(batch)) => {
                Some(batch.events.into_iter().map(|event| event.input))
            }
            _ => None,
        })
        .flatten()
        .collect()
}

fn publish<W: block_plugin_api::Watched>(instances: &mut Instances, value: &W::Value) -> bool {
    instances.publish(W::OPEN, Arc::new(block_plugin_api::ops::encode(value)))
}

fn watched_sent<W: block_plugin_api::Watched>(messages: &[Outbound]) -> Vec<W::Value> {
    messages
        .iter()
        .filter_map(|message| match message {
            Outbound::Editor(instance, EditorCall::Watched { open, value })
                if *instance == INSTANCE && *open == W::OPEN =>
            {
                block_plugin_api::ops::decode(value)
            }
            _ => None,
        })
        .collect()
}

fn watch<W: block_plugin_api::Watched>(instances: &mut Instances) -> bool {
    instances.watch_published(INSTANCE, W::OPEN)
}

fn host_call<W: block_plugin_api::Watched, A: serde::Serialize>(
    instances: &mut Instances,
    function: &str,
    args: &A,
) -> bool {
    instances.published_call(
        INSTANCE,
        block_plugin_api::ops::Called {
            name: format!("{}#{function}", W::INTERFACE),
            parent: None,
            answered: false,
            args: block_plugin_api::ops::encode(args),
        },
    )
}

fn configure(instances: &mut Instances) -> Option<PluginCall> {
    let next = instances.next_screens(PASS);
    let mut messages = next.opened;
    messages.push(PluginCall::Screens(instances.screen_set(next.screens)).into());
    instances.configure(&messages)
}

fn child(rect: Rect, follows_view: bool) -> ChildPlacements {
    ChildPlacements {
        instance: INSTANCE,
        region: REGION,
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
            top_bar: block_plugin_api::TopBar::Hidden,
            corner_radius: 0.0,
            layer: block_plugin_api::ChildLayer::Below,
            mode: ChildMode::Passive,
            intrinsic: None,
            rotation: 0.0,
            opacity: 1.0,
            follows_view,
            dialog: None,
            docks: None,
            in_layer: None,
        }],
        occluders: Vec::new(),
    }
}

fn next_screens(instances: &mut Instances) -> NextScreens {
    instances.touch(&block_app_stack::take_touched());
    instances.next_screens(PASS)
}

static NEXT_OP: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);

fn next_op() -> OpId {
    NEXT_OP.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
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

fn answered<T: block_plugin_api::ops::DeserializeOwned>(
    instances: &mut Instances,
    watch: OpId,
) -> Option<T> {
    let (_, bytes) = instances.answers(&[watch]).pop()?;
    Some(block_plugin_api::ops::decode(&bytes).expect("an answer decodes"))
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
            frame: block_type_api::encode_frame(message),
            reply: block_plugin_api::ops::Reply::new(next_op()),
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
            frames
                .extend(block_app_stack::unframe(frame).and_then(|bytes| ToEditor::decode(&bytes)));
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
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(20);
    let mut sent = Vec::new();
    loop {
        sent.extend(replica_sends(instances, block));
        if done(&sent) {
            return sent;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "the replica never sent what was expected: {sent:?}"
        );
        block_app_stack::wait_for(std::time::Duration::from_millis(100), |shared| {
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
        Some(block_plugin_api::FrameSpec::default()),
        SIZE,
        Rect::from_min_size(Pos2::ZERO, SIZE),
        1.0,
        1.0,
        false,
        None,
        Some(block_plugin_api::LayerSpace {
            to_host: block_plugin_api::HostTransform {
                origin,
                scale: vec2(1.0, 1.0),
            },
            bounds,
        }),
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
    instances.confirm(serial, &HashSet::new());
}

fn layer(id: u64, parent: block_plugin_api::LayerParent, rect: Rect) -> block_plugin_api::Layer {
    block_plugin_api::Layer {
        id: LayerId(id),
        parent,
        placement: block_plugin_api::LayerPlacement::Anchored(Rect::from_min_size(
            Pos2::ZERO,
            vec2(10.0, 10.0),
        )),
        rect,
        surface: 7,
        painted: vec![rect],
        floating: vec![rect],
        scrims: Vec::new(),
        dismiss_on_outside_press: true,
        keyboard: false,
        blocks_owner: false,
    }
}

fn report_layers(instances: &mut Instances, layers: Vec<block_plugin_api::Layer>) {
    let screen = instances
        .screen_id(INSTANCE, REGION)
        .expect("the region has a screen");
    instances.set_frame_reports(vec![block_plugin_api::FrameReport {
        screen,
        content: block_plugin_api::Insets::default(),
        painted: Vec::new(),
        floating: Vec::new(),
        floating_painted: Vec::new(),
        scrims: Vec::new(),
        drawing: Vec::new(),
        claims: Vec::new(),
        description: None,
        layers,
    }]);
}

fn shown_rects(instances: &Instances) -> Vec<Rect> {
    instances
        .layers(INSTANCE, REGION)
        .into_iter()
        .map(|layer| layer.rect)
        .collect()
}

mod a_block_is_named_after_its_content_until_someone_names_it;
mod a_block_shown_before_its_instance_reports_reaches_it_once_it_does;
mod a_child_placed_in_a_layer_is_shown_in_host_coordinates_inside_it;
mod a_click_outside_a_frame_child_hands_the_frame_back;
mod a_database_view_given_content_references_its_database;
mod a_frame_takeover_keeps_the_last_painting_where_it_was;
mod a_key_intercepted_for_a_plugin_reaches_it_with_its_modifiers;
mod a_layer_is_shown_from_the_report_that_opens_it_clipped_to_its_bounds;
mod a_listing_the_host_could_not_load_reaches_the_plugin_as_failed;
mod a_message_waits_for_the_instance_it_names_to_be_opened;
mod a_moved_region_moves_its_layers_with_it_and_a_pan_does_not;
mod a_plugin_hears_every_sample_only_while_it_asks;
mod a_plugin_is_told_when_the_pointer_leaves_it;
mod a_plugin_reaches_only_the_hosts_its_manifest_names;
mod a_plugin_seeds_only_blocks_it_holds;
mod a_press_outside_a_dismissable_chain_closes_it_and_what_hangs_from_it;
mod a_resize_leaves_children_where_their_frame_painted_them;
mod a_restarted_plugin_hears_its_childrens_statuses_again;
mod a_restarted_plugin_is_sent_its_content_and_blocks_again;
mod a_restarted_plugin_no_longer_shows_its_regions_stopped;
mod a_tap_intercepted_for_a_plugin_reaches_it_unless_it_has_the_keyboard;
mod a_watched_value_reaches_an_instance_watching_it_again_after_a_restart;
mod a_zoomed_region_is_shown_at_the_zoom_its_frame_was_painted_at;
mod after_the_first_snapshot_an_editor_is_sent_operations;
mod an_editor_can_read_and_edit_a_block_it_watches;
mod an_editor_is_only_sent_messages_its_plugin_session_accepts;
mod an_instance_inside_a_checkout_speaks_in_its_local_ids;
mod an_instance_the_plugin_never_opened_is_not_closed;
mod an_instance_watching_a_blocks_head_is_sent_its_stored_bytes;
mod an_instance_watching_a_blocks_history_is_told_when_it_changes;
mod children_laid_out_against_an_older_view_are_moved_to_the_current_one;
mod clearing_a_name_names_the_block_after_its_content_at_once;
mod closing_an_editor_calls_off_the_gesture_it_left_open;
mod host_calls_are_taken_by_kind_in_the_order_they_came;
mod input_is_withheld_from_screens_the_plugin_no_longer_has;
mod input_on_a_layer_reaches_its_owner_tagged_with_it_in_the_screen_s_coordinates;
mod revision_history_editors_cannot_mutate_any_block;
mod the_contacts_reach_only_the_shell;
mod the_view_a_screen_is_given_carries_its_scale;

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
        REGION,
        Uuid::nil(),
        role,
        &Arc::new(crate::PluginCatalog::default()),
        Some(block_plugin_api::FrameSpec::default()),
        size,
        Rect::from_min_size(Pos2::ZERO, size),
        scale_factor,
        PASS,
    );
}
