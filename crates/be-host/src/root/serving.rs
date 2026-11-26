//! What be-host serves the root under `--root` and `?root=` (`RootShell`) each frame, beside drawing it: the
//! host's catalog, notices and statistics, and the editors the root's apps open, their blocks,
//! children, quick open and the windows they show, which do not depend on where the root's
//! frames are drawn.

use beui::Clip;

use super::*;

/// The catalog a root was handed: the registry's block types, and the programs' editors as
/// they are pushed to it.
#[derive(Default)]
pub(super) struct HandedTypes {
    types: Option<std::sync::Arc<be_host_runtime::AppCatalog>>,
    programs: be_host_base::program_editors::Hearing,
}

/// Hands the root the host's catalog once it changed since it was `handed` (`host.block-types`):
/// the registry's block types, templates and editors with the editors of the programs listed,
/// as be-host-runtime tells its own apps, which name and draw block types from it.
pub(super) fn show_block_types(core: &crate::HostCore, root: &mut Root, handed: &mut HandedTypes) {
    let types = core.registry.app_block_types();
    let programs = handed.programs.take();
    let same = (handed.types.as_ref()).is_some_and(|handed| std::sync::Arc::ptr_eq(handed, types));
    if same && programs.is_none() {
        return;
    }
    let mut catalog = types.catalog.clone();
    catalog
        .editors
        .extend(programs.unwrap_or_else(be_host_base::program_editors::listed));
    root.show_block_types(
        be_sdk_api::ops::encode(&catalog),
        be_sdk_api::ops::encode(&types.debug_templates),
    );
    handed.types = Some(std::sync::Arc::clone(types));
}

/// Hands `root` the host's notices if they changed since the generation it was handed, `seen`.
pub(super) fn show_notices(root: &mut Root, seen: &mut Option<u64>) {
    let generation = notices::generation();
    if *seen != Some(generation) {
        *seen = Some(generation);
        root.show_notices(shown_notices());
    }
}

/// Hands `root` the host's statistics while it watches them (`host.watch-statistics`), and
/// measures be-host's memory only then, writing it nowhere: the sampling thread parks while
/// nothing watches and wakes the window for each sample while the root does, and each frame
/// hands the root what changed.
pub(super) fn show_statistics(root: &mut Root, memory: &mut Option<MemoryRows>) {
    let watching = root.watching_statistics();
    #[cfg(target_os = "linux")]
    {
        if watching {
            // Under `--root` it samples only while watched, and writes no file.
            crate::memory::start_watched();
        }
        crate::memory::watch(watching);
    }
    if !watching {
        *memory = None;
        return;
    }
    // Cached by be-host-base for a quarter of a second, so cheap to ask each frame.
    let performance = be_host_base::performance::rows()
        .into_iter()
        .map(statistic_row)
        .collect();
    // Read again only once a new sample came.
    #[cfg(target_os = "linux")]
    {
        let generation = crate::memory::generation();
        if memory
            .as_ref()
            .is_none_or(|shown| shown.generation != generation)
        {
            let view = crate::memory::view();
            // The graph's minutes, which change only with a new sample, so only then.
            let (points, now) = view
                .as_ref()
                .map(|view| {
                    let points = view
                        .points
                        .iter()
                        .map(|point| be_wasm_root_api::MemoryPoint {
                            at: point.at,
                            resident: point.usage.resident,
                            anonymous: point.usage.anonymous,
                            shared: point.usage.shared,
                            wasm: point.usage.wasm,
                        })
                        .collect();
                    (points, view.now)
                })
                .unwrap_or_default();
            root.show_memory_history(points, now);
            let (rows, file) = view
                .map(|view| {
                    let rows = view
                        .rows
                        .into_iter()
                        .map(|memory| statistic_row(memory.row));
                    (rows.collect(), view.file)
                })
                .unwrap_or_default();
            *memory = Some(MemoryRows {
                generation,
                rows,
                file,
            });
        }
    }
    let (rows, file) = memory
        .as_ref()
        .map(|shown| (shown.rows.clone(), shown.file.clone()))
        .unwrap_or_default();
    root.show_statistics(performance, rows, file);
}

/// The memory rows the root was last handed, and the sample they are of.
pub(super) struct MemoryRows {
    #[cfg_attr(
        not(target_os = "linux"),
        expect(dead_code, reason = "only Linux samples memory")
    )]
    generation: u64,
    rows: Vec<StatisticRow>,
    file: Option<String>,
}

/// One of be-host-base's performance rows, or a memory row, as `be3:root` carries it.
pub(super) fn statistic_row(row: be_host_base::performance::PerformanceRow) -> StatisticRow {
    StatisticRow {
        heading: row.heading,
        name: row.name,
        current: row.current,
        average: row.average,
        peak: row.peak,
    }
}

/// Serves the root the editors of the blocks it shows (plans/host_as_platform_adapter.md,
/// 4g7b1): names the app that edits each block it asks of (`host.editor-of`), opens the
/// editor in the app it spawned for it (`host.open-editor`) with be-host-runtime's
/// `Forwarded`, one for each of the root's instances, and serves what each app's editors ask
/// through the root (`host.forward`, `host.forwarded`) as be-host serves its own apps. An
/// instance the root let go of, or that stopped, it stops serving.
pub(super) fn serve_editors(core: &mut crate::HostCore, root: &mut Root, editors: &mut Editors) {
    let opens = std::mem::take(&mut core.root_opens);
    let served = &mut editors.served;
    // What editors asked the shell to show, which the session's editor the root embeds shows
    // as be-host's own shell does; with no shell open yet, nothing shows it.
    for (block, dialog) in std::mem::take(&mut core.root_dialogs) {
        served
            .values_mut()
            .any(|forwarded| forwarded.show_dialog(block, dialog.clone()));
    }
    let asks: Vec<_> = std::mem::take(&mut editors.asks)
        .into_iter()
        .chain(root.take_editor_asks())
        .collect();
    let openings: Vec<_> = std::mem::take(&mut editors.openings)
        .into_iter()
        .chain(root.take_openings())
        .collect();
    // The stack lists what the root asks for and every block whose editor it serves the root,
    // as it lists the blocks be-host's own editors show: it may not have them yet once it
    // starts on the data it kept, and keeps them while they show.
    let shown_block = |shown: &be_wasm_root_api::Shown| match shown {
        be_wasm_root_api::Shown::Block(shown) => Some(shown.block),
        be_wasm_root_api::Shown::Dialog(_) => None,
    };
    let mut wanted: Vec<[u8; 16]> = asks
        .iter()
        .filter_map(|(_, shown)| shown_block(shown))
        .chain(
            openings
                .iter()
                .filter_map(|opening| shown_block(&opening.shown)),
        )
        .chain(
            editors
                .opened
                .values()
                .filter_map(|opened| opened.block.map(Uuid::into_bytes)),
        )
        // The session's block from the frame the host chose it, before the root asks for its
        // editor: listed only by what is open meanwhile, a change to that would drop it.
        .chain(core.root_session.map(Uuid::into_bytes))
        .collect();
    wanted.sort_unstable();
    wanted.dedup();
    be_host_stack::want(
        be_host_stack::Owner::Root,
        wanted
            .into_iter()
            .map(|block| be_host_stack::Query::Block(Uuid::from_bytes(block)))
            .collect(),
    );
    for (id, shown) in asks {
        // While the stack does not list the session's block (yet, or again for a moment), or
        // has yet to read the listing of the block asked for, an ask for a block it does not
        // list waits for it rather than failing.
        if waits_for_listing(core, &shown) {
            editors.asks.push((id, shown));
            continue;
        }
        let editor = shown_editor(core, &shown).map(|(app, editor, _)| EditorApp {
            app: app.identity.id.clone(),
            module: app.entry_point.clone(),
            editor: editor.into_bytes(),
            name: app.identity.name.clone(),
            preview: app
                .editor(editor.into_bytes())
                .is_some_and(|editor| editor.views.contains(&ParentView::Preview)),
            pan_and_zoom: app
                .editor(editor.into_bytes())
                .is_some_and(|editor| editor.capabilities.pan_and_zoom),
            // The desktop shell's windows float on the desktop, as be-host's own UI floats them
            // (`compositor/desktop.rs`).
            hosts_windows: core.desktop && editor == be_block::DESKTOP_EDITOR,
        });
        root.answer_editor(id, editor);
    }
    for opening in openings {
        if waits_for_listing(core, &opening.shown) {
            editors.openings.push(opening);
            continue;
        }
        let opened = open_editor(core, &opening, served, &mut editors.opened);
        root.answer_opening(opening.id, opened);
    }
    for (instance, frames) in root.take_forwarded() {
        let Some(forwarded) = served.get_mut(&instance) else {
            continue;
        };
        match forwarded.receive(frames) {
            Ok(frames) => root.relay(instance, frames),
            Err(why) => {
                // Stopped toward the root too, which shows that it stopped.
                if let Some(mut forwarded) = served.remove(&instance) {
                    forwarded.close_all();
                }
                root.stop_instance(instance, &format!("it broke the app protocol: {why}"));
            }
        }
    }
    served.retain(|instance, forwarded| {
        let kept = root.app_of(*instance).is_some();
        if !kept {
            forwarded.close_all();
        }
        kept
    });
    editors.opened.retain(|(instance, editor), opened| {
        let kept = served.contains_key(instance);
        if !kept {
            forget_opened(core, *editor, opened);
        }
        kept
    });
    // What the root's dialogs told the shell, which their placers' statuses say: those of
    // every view the root showed are worded again with it.
    let mut told = false;
    for (instance, editor, messages) in root.take_shell_tells() {
        // A root that encodes what no editor can read has nothing to relay.
        let Some(messages) = be_sdk_api::ops::decode::<Vec<ShellMessage>>(&messages) else {
            continue;
        };
        // The program windows the shell placed that it asks to close, as be-host's own shell
        // closes them (`close_children`); a close of any other child is the root's.
        let (closes, messages): (Vec<_>, Vec<_>) = messages
            .into_iter()
            .partition(|message| matches!(message, ShellMessage::CloseChild(_)));
        for close in closes {
            if let ShellMessage::CloseChild(child) = close {
                close_window(core, &editors.shown, served, (instance, editor), child);
            }
        }
        // Only a dialog's placer's status says what it told: a block's editor's are dropped.
        let dialog = editors
            .opened
            .get(&(instance, editor))
            .is_some_and(|opened| opened.block.is_none());
        let Some(forwarded) = served.get_mut(&instance).filter(|_| dialog) else {
            continue;
        };
        if !messages.is_empty() {
            let committed = messages
                .iter()
                .any(|message| matches!(message, ShellMessage::CommitChild(_)));
            // The commit of a creation its placer sent, which makes its block.
            if let Some(creation) = editors
                .opened
                .get_mut(&(instance, editor))
                .and_then(|opened| opened.creation.as_mut())
            {
                for message in &messages {
                    if let ShellMessage::CommitChild(commit) = message {
                        creation.commit = Some(be_host_runtime::ChildCommit {
                            child: commit.child,
                            parent: commit.parent,
                            name: commit.name.clone(),
                        });
                    }
                }
            }
            // Heard first, so a draft told with the commit is the one applied.
            forwarded.shell_told(EditorInstanceId(editor), messages);
            // Accepted artifact settings its placer committed, which the host applies.
            if let Some(artifact) = editors
                .opened
                .get(&(instance, editor))
                .and_then(|opened| opened.artifact)
                && committed
                && let Some(draft) = forwarded.artifact_draft(EditorInstanceId(editor))
            {
                editors.artifact_applies.push((artifact, draft));
            }
            told = true;
        }
    }
    told |= step_creations(&mut editors.opened, served);
    let mut views = Vec::new();
    for shown in root.take_shown_children() {
        let placer = (shown.instance, shown.editor);
        editors.placers.retain(|_, held| *held != placer);
        for child in &shown.children {
            if let Some(opened) = &child.shown_in {
                editors
                    .placers
                    .insert((opened.instance, opened.editor), placer);
            }
        }
        let view = (shown.instance, shown.editor, shown.view);
        match shown.children.is_empty() {
            true => editors.shown.remove(&view),
            false => editors.shown.insert(view, shown.children),
        };
        views.push(view);
    }
    editors
        .shown
        .retain(|(instance, ..), _| served.contains_key(instance));
    if told {
        views = editors.shown.keys().copied().collect();
    }
    // The views placing program windows whose statuses changed since they were last worded.
    if core.desktop {
        views.extend(windows_restated(
            &editors.shown,
            served,
            &mut editors.windows,
        ));
    }
    views.sort_unstable();
    views.dedup();
    for (instance, editor, view) in views {
        let by_shell = served
            .get(&instance)
            .is_some_and(|forwarded| forwarded.is_shell(EditorInstanceId(editor)));
        let statuses = editors
            .shown
            .get(&(instance, editor, view))
            .map(|children| {
                children
                    .iter()
                    .map(|child| child_status(core, &editors.opened, served, child, by_shell))
                    .collect()
            })
            .unwrap_or_default();
        if let Some(forwarded) = served.get_mut(&instance) {
            forwarded.show_children(EditorInstanceId(editor), ViewId(view), statuses);
        }
    }
    for (instance, editor, event) in root.take_editor_events() {
        let Some(forwarded) = served.get_mut(&instance) else {
            continue;
        };
        // A root that encodes what no editor can read has nothing to tell it.
        if let Some(event) = be_sdk_api::ops::decode::<EditorEvent>(&event) {
            forwarded.tell(EditorInstanceId(editor), event);
        }
    }
    for (instance, editor, panel) in root.take_panel_shows() {
        let Some(forwarded) = served.get_mut(&instance) else {
            continue;
        };
        // A root that encodes what no editor can read asks for nothing.
        if let Some(panel) = be_sdk_api::ops::decode::<be_sdk_api::HostPanel>(&panel) {
            forwarded.show_panel(EditorInstanceId(editor), panel);
        }
    }
    for (instance, editor, updates) in root.take_dock_updates() {
        let Some(forwarded) = served.get_mut(&instance) else {
            continue;
        };
        // A root that encodes what no editor can read has nothing to hand it.
        if let Some(updates) = be_sdk_api::ops::decode::<Vec<be_sdk_api::DockUpdate>>(&updates) {
            forwarded.dock(EditorInstanceId(editor), updates);
        }
    }
    for placed in root.take_placed_contents() {
        place_content(editors, placed);
    }
    for (instance, editor) in root.take_closed_editors() {
        if let Some(forwarded) = editors.served.get_mut(&instance) {
            forwarded.close(EditorInstanceId(editor));
        }
        if let Some(opened) = editors.opened.remove(&(instance, editor)) {
            forget_opened(core, editor, &opened);
        }
    }
    for (instance, editor, size) in root.take_assigned_sizes() {
        if let Some(forwarded) = editors.served.get_mut(&instance) {
            forwarded.resized(
                EditorInstanceId(editor),
                beui::vec2(size.width, size.height),
            );
        }
    }
    editors
        .placers
        .retain(|child, _| editors.opened.contains_key(child));
    open_blocks(core, opens, editors);
    quick_open(core, root, editors);
    reveal_opened(editors);
    // The names of the blocks the root asks after, as the graph holds them.
    for (id, blocks) in root.take_name_asks() {
        let names = blocks
            .into_iter()
            .map(|block| {
                be_host_stack::node(Uuid::from_bytes(block))?
                    .metadata
                    .name()
            })
            .collect();
        root.answer_names(id, names);
    }
    show_artifact_states(core, editors);
    for (instance, forwarded) in editors.served.iter_mut() {
        root.relay(*instance, forwarded.step());
    }
}

/// Hands each editor of the root's apps that watches artifacts their states as be-host made
/// them for every editor watching (`HostCore::poll_artifacts`), as it hands its own.
pub(super) fn show_artifact_states(core: &crate::HostCore, editors: &mut Editors) {
    let Editors { served, opened, .. } = editors;
    for ((instance, editor), opened) in opened.iter() {
        let editor = EditorInstanceId(*editor);
        let watches = core
            .artifact_watches
            .contains_key(&(opened.app.identity.id.clone(), editor));
        if let (true, Some(forwarded)) = (watches, served.get_mut(instance)) {
            forwarded.set_artifact_states(editor, core.artifact_states.clone());
        }
    }
}

/// Hands the editor `placed` names, shown as a placed child (`host.place-content`), the part of
/// its world it shows, if it pans and zooms, as be-host's camera does for a placed child
/// (`Camera`'s `shown`): where its content shows, at that rect's scale over the size its content
/// wants.
pub(super) fn place_content(editors: &mut Editors, placed: be_wasm_root_api::PlacedContent) {
    let pans = editors
        .opened
        .get(&(placed.instance, placed.editor))
        .is_some_and(|opened| {
            opened
                .app
                .editor(opened.editor.into_bytes())
                .is_some_and(|editor| editor.capabilities.pan_and_zoom)
        });
    let Some(forwarded) = editors.served.get_mut(&placed.instance).filter(|_| pans) else {
        return;
    };
    let content = placed.content;
    let rect = beui::Rect::from_min_max(
        beui::pos2(content.min_x, content.min_y),
        beui::pos2(content.max_x, content.max_y),
    );
    let scale = placed
        .intrinsic
        .filter(|intrinsic| intrinsic.width > 0.0 && intrinsic.height > 0.0)
        .map_or(1.0, |intrinsic| {
            (rect.width() / intrinsic.width).min(rect.height() / intrinsic.height)
        });
    forwarded.set_view(
        EditorInstanceId(placed.editor),
        be_host_runtime::EditorView { rect, scale },
    );
}

/// Hands each block the root's editors asked to open (`opens`, by the app and instance that
/// asked) to the editor that handles show requests, as be-host's `show_block` does: the nearest
/// of the asking editor and those that place it whose manifest accepts them, else the editor
/// showing the session's block, which be-host's own shell is. An ask from an editor the root
/// no longer shows goes to the editor showing the session's block, as be-host hands one from
/// no editor it knows to its shell; with no session shown, it is dropped.
pub(super) fn open_blocks(
    core: &crate::HostCore,
    opens: Vec<(String, EditorInstanceId, be_host_runtime::BlockOpen)>,
    editors: &mut Editors,
) {
    for (app, instance, open) in opens {
        let from = editors
            .opened
            .iter()
            .find(|((_, editor), opened)| *editor == instance.0 && opened.app.identity.id == app)
            .map(|(key, _)| *key);
        open_block(core, editors, from, open);
    }
}

/// Opens `open` as the editor `from` (the root's instance and the editor) asked to, or as the
/// session's editor would for none: the nearest editor placing it that shows blocks shows it.
pub(super) fn open_block(
    core: &crate::HostCore,
    editors: &mut Editors,
    from: Option<(u64, u64)>,
    open: be_host_runtime::BlockOpen,
) {
    let shell = editors
        .opened
        .iter()
        .find(|(key, opened)| {
            opened
                .block
                .is_some_and(|block| Some(block) == core.root_session)
                && !editors.placers.contains_key(key)
        })
        .map(|(key, _)| *key);
    let Some(handler) = handler(from, shell, editors) else {
        return;
    };
    // Where it is opened at (a search hit), which its editor is told once it opens, as be-host
    // tells its own (`HostCore::show_block`).
    editors
        .reveals
        .opened(open.id, open.at.clone(), be_host_base::host::now());
    if let Some(forwarded) = editors.served.get_mut(&handler.0) {
        forwarded.show_block(EditorInstanceId(handler.1), open);
    }
}

/// Tells each block opened at a place (a search hit's) that place (`editor-event.reveal`) once
/// an editor the root shows it in is open, as be-host tells its own (`HostCore::reveal_opened`).
pub(super) fn reveal_opened(editors: &mut Editors) {
    if editors.reveals.is_empty() {
        return;
    }
    let mut reveals = std::mem::take(&mut editors.reveals);
    reveals.deliver(be_host_base::host::now(), |block, at| {
        let showing: Vec<(u64, u64)> = editors
            .opened
            .iter()
            .filter(|(_, opened)| opened.block == Some(block))
            .map(|(key, _)| *key)
            .collect();
        let mut delivery = crate::reveals::Delivery::NoEditor;
        for (instance, editor) in showing {
            let Some(forwarded) = editors.served.get_mut(&instance) else {
                continue;
            };
            if forwarded.reveal(EditorInstanceId(editor), at.clone()) {
                return crate::reveals::Delivery::Told;
            }
            delivery = crate::reveals::Delivery::Opening;
        }
        delivery
    });
    editors.reveals = reveals;
}

/// Answers the root's quick open (`host.find-blocks`) as be-host's own palette finds blocks
/// (`compositor::quick_open`): at once for no text, else once the search index answered what is
/// typed, which is wanted until then; and opens the blocks it chose (`host.open-block`) as the
/// editor it opened from asked to.
pub(super) fn quick_open(core: &crate::HostCore, root: &mut Root, editors: &mut Editors) {
    // Those the root stopped waiting for go first, and their searches with them.
    let cancelled = root.take_cancelled_finds();
    editors.finds.retain(|(id, _)| !cancelled.contains(id));
    editors.finds.extend(root.take_block_finds());
    let wanted: Vec<be_host_stack::SearchAsk> = editors
        .finds
        .iter()
        .filter_map(|(_, text)| crate::compositor::search_ask(text))
        .collect();
    be_host_stack::want_searches(be_host_stack::Owner::Root, wanted);
    let (ready, waiting): (Vec<_>, Vec<_>) = std::mem::take(&mut editors.finds)
        .into_iter()
        .partition(|(_, text)| crate::compositor::quick_open_ready(text));
    editors.finds = waiting;
    if !ready.is_empty() {
        // The blocks the root shows in an editor, which it lists first; read once for them all.
        let open: Vec<Uuid> = editors
            .opened
            .values()
            .filter_map(|opened| opened.block)
            .collect();
        let blocks = crate::compositor::quick_open_blocks(&open);
        for (id, text) in ready {
            let found = crate::compositor::quick_open(&blocks, &text);
            root.answer_block_find(id, found.into_iter().map(openable).collect());
        }
    }
    for (from, block) in root.take_block_opens() {
        let open = be_host_runtime::BlockOpen {
            id: Uuid::from_bytes(block.block),
            block_type: Uuid::from_bytes(block.block_type),
            via: None,
            editor: None,
            // A root that encodes what no host can read opens the block where it opens.
            at: be_sdk_api::ops::decode(&block.at),
        };
        open_block(
            core,
            editors,
            from.map(|from| (from.instance, from.editor)),
            open,
        );
    }
}

/// A block quick open offers, as the root hears it.
pub(super) fn openable(block: crate::compositor::OpenableBlock) -> be_wasm_root_api::OpenableBlock {
    be_wasm_root_api::OpenableBlock {
        block: block.id.into_bytes(),
        block_type: block.block_type.into_bytes(),
        name: block.name,
        detail: block.detail,
        at: block
            .at
            .as_ref()
            .map(be_sdk_api::ops::encode)
            .unwrap_or_default(),
    }
}

/// The editor that handles a show request from `from` (be-host's `shell_route::handler`): the
/// nearest of it and the editors placing it, up to the `shell`, whose manifest accepts show
/// requests, else the shell.
pub(super) fn handler(
    from: Option<(u64, u64)>,
    shell: Option<(u64, u64)>,
    editors: &Editors,
) -> Option<(u64, u64)> {
    let accepts = |key: &(u64, u64)| {
        editors.opened.get(key).is_some_and(|opened| {
            opened
                .app
                .editor(opened.editor.into_bytes())
                .is_some_and(|editor| editor.accepts(be_sdk_manifest::ShellRequest::ShowBlock))
        })
    };
    let mut visited = Vec::new();
    let mut current = from;
    while let Some(key) = current {
        if Some(key) == shell || visited.contains(&key) {
            break;
        }
        if accepts(&key) {
            return Some(key);
        }
        visited.push(key);
        current = editors.placers.get(&key).copied();
    }
    shell
}

/// Whether an ask naming `shown` waits for the stack to list its block (`unlisted_yet`): a
/// dialog names none.
pub(super) fn waits_for_listing(core: &crate::HostCore, shown: &be_wasm_root_api::Shown) -> bool {
    match shown {
        be_wasm_root_api::Shown::Block(shown) => unlisted_yet(core, shown.block),
        be_wasm_root_api::Shown::Dialog(_) => false,
    }
}

/// What a child the root opens an editor for shows, as the host opens it (`shown_editor`).
pub(super) enum Shows {
    Block(crate::editors::Opened),
    Picker(be_host_runtime::PickerRequest),
    Rename(Uuid, bool),
    Export(EditorBlock, be_sdk_api::ExportRequest),
    Creation(crate::editors::app::CreationTarget),
    /// An artifact's settings: the artifact, and the exporter and settings that make it.
    ArtifactSettings(EditorBlock, Uuid, String, Vec<u8>),
}

/// The app and editor that run what the root's child shows, and how it is opened: a block's
/// editor (`HostCore::editor_of`), or the editor that runs a dialog an editor placed, as
/// be-host's registry chooses it (`picker`, `rename`, `export`).
pub(super) fn shown_editor(
    core: &crate::HostCore,
    shown: &be_wasm_root_api::Shown,
) -> Result<(std::sync::Arc<be_sdk_manifest::AppManifest>, Uuid, Shows), String> {
    let (content, sheet) = match shown {
        be_wasm_root_api::Shown::Block(shown) => {
            return core
                .editor_of(shown)
                .map(|(app, editor, opened)| (app, editor, Shows::Block(opened)));
        }
        be_wasm_root_api::Shown::Dialog(dialog) => (
            be_sdk_api::ops::decode::<be_sdk_api::ChildContent>(&dialog.content)
                .ok_or("the root named a dialog the host cannot read")?,
            dialog.sheet,
        ),
    };
    match content {
        be_sdk_api::ChildContent::Picker { filter, parent } => {
            let (app, editor) = core
                .registry
                .accepting(be_sdk_manifest::ShellRequest::Pick)
                .ok_or("no app picks blocks")?;
            let request = be_host_runtime::PickerRequest {
                filter,
                parent,
                sheet,
            };
            Ok((app, editor, Shows::Picker(request)))
        }
        be_sdk_api::ChildContent::Rename(block) => {
            let (app, editor) = core
                .registry
                .accepting(be_sdk_manifest::ShellRequest::Rename)
                .ok_or("no app renames blocks")?;
            Ok((app, editor, Shows::Rename(Uuid::from_bytes(block), sheet)))
        }
        be_sdk_api::ChildContent::Export { block_id, request } => {
            let block = Uuid::from_bytes(block_id);
            let node = be_host_stack::node(block).ok_or("no block of that id is listed")?;
            let (app, editor) = core
                .registry
                .exporting(node.content_type, &request.exporter)
                .ok_or("the block's editor has no such exporter")?;
            let opened = EditorBlock {
                id: block,
                block_type: node.content_type,
                view_block: None,
            };
            Ok((app, editor, Shows::Export(opened, request)))
        }
        be_sdk_api::ChildContent::ArtifactSettings { block_id } => {
            let block = Uuid::from_bytes(block_id);
            let artifact = crate::artifact_of(block).ok_or("the block is no artifact")?;
            let node = be_host_stack::node(block).ok_or("no block of that id is listed")?;
            let (app, editor) = core
                .registry
                .artifact_editor(artifact.source_type, &artifact.exporter)?;
            let opened = EditorBlock {
                id: block,
                block_type: node.content_type,
                view_block: None,
            };
            Ok((
                app,
                editor,
                Shows::ArtifactSettings(
                    opened,
                    *artifact.source,
                    artifact.exporter,
                    artifact.settings,
                ),
            ))
        }
        be_sdk_api::ChildContent::Creation { editor, template } => {
            let (app, target) = core
                .registry
                .creation_app(Uuid::from_bytes(editor), &template)
                .ok_or("no app makes blocks from that template")?;
            Ok((app, target.editor, Shows::Creation(target)))
        }
        _ => Err("the host runs no dialog of that kind".to_owned()),
    }
}

/// Whether an ask for an editor of `block` waits: the stack does not list it, but may yet, while
/// the session's block is not listed or the stack has yet to read the block's listing, which
/// `serve_editors` asks it for.
pub(super) fn unlisted_yet(core: &crate::HostCore, block: [u8; 16]) -> bool {
    let block = Uuid::from_bytes(block);
    let query = be_host_stack::Query::Block(block);
    let reading = !be_host_stack::ready(query) && be_host_stack::failure(query).is_none();
    (core.root_session_pending || reading) && be_host_stack::node(block).is_none()
}

/// The editors be-host serves the root's apps, by the root's instance each runs in, and the
/// root's asks for editors of blocks the stack does not list yet, which wait until it does.
#[derive(Default)]
pub(super) struct Editors {
    pub(super) served: HashMap<u64, Forwarded>,
    /// Each editor opened, by the root's instance and the editor: what its app's manifest says
    /// of it, for the statuses of the children it shows.
    pub(super) opened: HashMap<(u64, u64), OpenedFor>,
    pub(super) asks: Vec<(u64, be_wasm_root_api::Shown)>,
    pub(super) openings: Vec<EditorOpening>,
    /// What the root's quick open asked to find (`host.find-blocks`) that waits for the search
    /// index, by the id it is answered by.
    pub(super) finds: Vec<(u64, String)>,
    /// The children each view of the editors the root shows placed, as the root last said
    /// (`host.show-children`), by the root's instance, the editor and the view: what their
    /// statuses are worded from again once a dialog among them told the shell something.
    pub(super) shown: HashMap<(u64, u64, u64), Vec<be_wasm_root_api::ShownChild>>,
    /// The statuses of the program windows each view places, as they were last worded: a view
    /// is worded again once one of its windows' changes.
    pub(super) windows: HashMap<(u64, u64, u64), Vec<Option<WindowStatus>>>,
    /// The editor placing each editor the root shows as a child, as the root last said
    /// (`host.show-children`), each by the root's instance and the editor: what a show request
    /// climbs to find its handler.
    pub(super) placers: HashMap<(u64, u64), (u64, u64)>,
    /// The settings artifact settings children accepted, by the artifact, for the host to apply
    /// (`HostCore::store_artifact_settings`).
    pub(super) artifact_applies: Vec<(Uuid, Vec<u8>)>,
    /// Places blocks were opened at (a search hit's), held until an editor showing the block
    /// is open.
    pub(super) reveals: crate::reveals::Reveals,
}

/// Forgets what the editor `editor` watched and, for a dialog, what it was opened to run
/// (`ask_picker`, `ask_export`) once it closes, as be-host does when its own close.
pub(super) fn forget_opened(core: &mut crate::HostCore, editor: u64, opened: &OpenedFor) {
    core.forget_artifact_watch(opened.app.identity.id.clone(), EditorInstanceId(editor));
    if opened.block.is_none() {
        be_host_runtime::forget_picker(EditorInstanceId(editor));
        be_host_runtime::forget_export(EditorInstanceId(editor));
    }
}

/// An editor be-host opened in an app the root spawned: the app's manifest, the editor it opened
/// and its block, none for a dialog (a picker, a rename form, an Export dialog).
pub(super) struct OpenedFor {
    app: std::sync::Arc<be_sdk_manifest::AppManifest>,
    editor: Uuid,
    block: Option<Uuid>,
    /// For a creation dialog (`InstanceRole::Creation`), how its block is being made.
    creation: Option<RootCreation>,
    /// For an artifact's settings, the artifact.
    artifact: Option<Uuid>,
}

/// A creation a root's child runs, as be-host's `CreationChild` steps one: the template, the
/// commit its placer sent (`commit-child`), and how far making its block got.
pub(super) struct RootCreation {
    target: crate::editors::app::CreationTarget,
    commit: Option<be_host_runtime::ChildCommit>,
    /// Whether its editor was asked to make the block (`Forwarded::commit_creation`).
    committed: bool,
    progress: Option<be_sdk_api::CreationProgress>,
    /// The block its editor made, or why not, as it answered (`InstanceEvent::Created`).
    created: crate::editors::app::Inbox<Result<Uuid, String>>,
}

/// Steps each creation a root's child runs as be-host steps its own (`step_creation`): once
/// its editor is open (and a dialog accepted), asks it to make the block. Answers whether any
/// changed how far it got, which its placer's status says.
pub(super) fn step_creations(
    opened: &mut HashMap<(u64, u64), OpenedFor>,
    served: &mut HashMap<u64, Forwarded>,
) -> bool {
    let mut changed = false;
    for ((instance, editor), opened) in opened.iter_mut() {
        let Some(creation) = opened.creation.as_mut() else {
            continue;
        };
        let Some(forwarded) = served.get_mut(instance) else {
            continue;
        };
        let editor = EditorInstanceId(*editor);
        let RootCreation {
            target,
            commit,
            committed,
            progress,
            created,
        } = creation;
        changed |= crate::editors::step_creation(target.name, commit, progress, || {
            let ready = match target.dialog {
                true => {
                    forwarded.dialog_status(editor).answer
                        == Some(be_sdk_api::DialogAnswer::Accepted)
                }
                false => forwarded.is_open(editor),
            };
            if !ready {
                return Ok(None);
            }
            if !*committed {
                *committed = true;
                forwarded.commit_creation(editor);
            }
            match created.take_latest() {
                None => Ok(None),
                Some(Ok(block)) => Ok(Some(block)),
                Some(Err(error)) => {
                    *committed = false;
                    Err(error)
                }
            }
        });
    }
    changed
}

/// Closes the program window the child `child` of `placer` (the root's instance and the editor)
/// shows, as be-host's own shell closes one (`close_children`): only the shell places windows.
pub(super) fn close_window(
    core: &crate::HostCore,
    shown: &HashMap<(u64, u64, u64), Vec<be_wasm_root_api::ShownChild>>,
    served: &HashMap<u64, Forwarded>,
    placer: (u64, u64),
    child: ChildId,
) {
    let (instance, editor) = placer;
    let by_shell = served
        .get(&instance)
        .is_some_and(|forwarded| forwarded.is_shell(EditorInstanceId(editor)));
    if !core.desktop || !by_shell {
        return;
    }
    let window = shown
        .iter()
        .filter(|((at, by, _), _)| (*at, *by) == placer)
        .flat_map(|(_, children)| children)
        .find(|shown| shown.child == child.0)
        .and_then(|shown| shown.window);
    if let Some(window) = window {
        Host::close(HostWindowId(window));
    }
}

/// What a program window's status says to its placer, as the host keeps it: its status less
/// the size it was last drawn at, which its placer hears nothing of.
pub(super) fn window_status(window: u64) -> Option<WindowStatus> {
    Host::window_status(HostWindowId(window)).map(|status| WindowStatus {
        drawn: beui::Vec2::ZERO,
        ..status
    })
}

/// The views of the shell placing program windows whose statuses changed since they were last
/// worded, which it notes as worded; it forgets the views no longer shown.
pub(super) fn windows_restated(
    shown: &HashMap<(u64, u64, u64), Vec<be_wasm_root_api::ShownChild>>,
    served: &HashMap<u64, Forwarded>,
    worded: &mut HashMap<(u64, u64, u64), Vec<Option<WindowStatus>>>,
) -> Vec<(u64, u64, u64)> {
    worded.retain(|view, _| shown.contains_key(view));
    let mut changed = Vec::new();
    for (view, children) in shown {
        // Only the shell hears its windows' statuses.
        let (instance, editor, _) = *view;
        if !served
            .get(&instance)
            .is_some_and(|forwarded| forwarded.is_shell(EditorInstanceId(editor)))
        {
            continue;
        }
        let statuses: Vec<_> = children
            .iter()
            .filter_map(|child| child.window)
            .map(window_status)
            .collect();
        if statuses.is_empty() {
            worded.remove(view);
            continue;
        }
        if worded.get(view) != Some(&statuses) {
            worded.insert(*view, statuses);
            changed.push(*view);
        }
    }
    changed
}

/// Said of a child the root shows nothing of and gave no reason for: one whose editor is still
/// being opened.
pub(super) const CHILD_OPENING: &str = "the block's editor is opening";

/// The status of a child an editor the root embeds placed, as the root shows it
/// (`host.show-children`): available once an editor shows it, with what its app's manifest
/// says of that editor (be-host's `status_of`, for its own children). A preview is available
/// at once, as under be-host (`preview_of`): waiting while its editor opens, then shown in its
/// place by the root, or drawn as its block's label by its placer when its editor draws none.
pub(super) fn child_status(
    core: &crate::HostCore,
    opened: &HashMap<(u64, u64), OpenedFor>,
    served: &HashMap<u64, Forwarded>,
    child: &be_wasm_root_api::ShownChild,
    by_shell: bool,
) -> HostChildStatus {
    if let Some(window) = child.window {
        return window_child_status(core, child, window, by_shell);
    }
    let shown = child
        .shown_in
        .as_ref()
        .and_then(|shown| opened.get(&(shown.instance, shown.editor)));
    let manifest = shown.and_then(|shown| shown.app.editor(shown.editor.into_bytes()));
    // Until its editor is open, the editor that will show it says how it is placed (its
    // interaction, capabilities and resizing), as be-host knows it at once.
    let edits = child
        .shows
        .as_ref()
        .filter(|_| manifest.is_none())
        .and_then(|shows| shown_editor(core, shows).ok());
    let described = manifest.or_else(|| {
        edits
            .as_ref()
            .and_then(|(app, editor, _)| app.editor(editor.into_bytes()))
    });
    let preview = match (child.preview, manifest) {
        (ShownPreview::None, _) => ChildPreview::Hosted,
        (ShownPreview::Label, _) => ChildPreview::Label,
        (ShownPreview::View, Some(_)) => ChildPreview::Composited,
        (ShownPreview::View, None) => ChildPreview::Waiting,
    };
    // A dialog (a picker, a rename form, an Export dialog) is available at once, as be-host's
    // own are: its placer shows its frame while its editor opens.
    let dialog_shown = matches!(child.shows, Some(be_wasm_root_api::Shown::Dialog(_)));
    let available = manifest.is_some()
        || (dialog_shown && child.error.is_none())
        || (child.preview != ShownPreview::None && child.error.is_none());
    // A dialog's pick, answer and card, as what it told the shell that the root relays says
    // (`host.tell-shell`), as be-host's `picker_status` says them.
    let dialog = child
        .shown_in
        .as_ref()
        .filter(|_| shown.is_some_and(|shown| shown.block.is_none()))
        .and_then(|shown| {
            let forwarded = served.get(&shown.instance)?;
            let editor = EditorInstanceId(shown.editor);
            Some((
                forwarded.picked(editor),
                forwarded.dialog_status(editor),
                forwarded.card(editor),
            ))
        });
    let error = shown
        .and_then(|shown| shown.block)
        .and_then(be_host_stack::revisions::error)
        .or_else(|| child.error.clone())
        .or_else(|| (!available).then(|| CHILD_OPENING.to_owned()));
    HostChildStatus {
        child: ChildId(child.child),
        available,
        intrinsic: child
            .intrinsic
            .as_ref()
            .map(|size| beui::vec2(size.width, size.height)),
        active: available && child.active,
        interaction: described.map_or(InteractionMode::Preview, |editor| editor.interaction),
        capabilities: described
            .map_or_else(EditorCapabilities::default, |editor| editor.capabilities),
        resize: described.map_or(ResizeMode::None, |editor| editor.resize),
        error,
        // A root that encodes what no editor can read shows the child with no menu.
        menu: be_sdk_api::ops::decode::<Vec<be_sdk_api::MenuItem>>(&child.menu)
            .and_then(|items| be_sdk_api::MenuEntry::nest(items).ok())
            .unwrap_or_default(),
        // How far a creation's block got, as be-host's `creation_status` says: one whose editor
        // failed could not make it, as be-host says of a template that fails to open.
        creation: child
            .shown_in
            .as_ref()
            .and_then(|shown| opened.get(&(shown.instance, shown.editor)))
            .and_then(|opened| opened.creation.as_ref())
            .and_then(|creation| creation.progress.clone())
            .or_else(|| match (&child.error, &edits) {
                (Some(error), Some((_, _, Shows::Creation(target)))) => {
                    Some(be_sdk_api::CreationProgress::Failed(
                        crate::editors::creation_failed(target.name, error),
                    ))
                }
                _ => None,
            }),
        preview,
        // A host panel the root draws says it is done as be-host's own does: its dialog
        // accepted, which closes the popover holding it.
        dialog: match &child.shows {
            Some(be_wasm_root_api::Shown::Dialog(shown)) if shown.done => DialogStatus {
                answer: Some(be_sdk_api::DialogAnswer::Accepted),
                ..DialogStatus::default()
            },
            _ => dialog
                .as_ref()
                .map_or_else(DialogStatus::default, |(_, dialog, _)| *dialog),
        },
        picked: dialog.as_ref().and_then(|(picked, ..)| picked.clone()),
        card: dialog.as_ref().is_some_and(|(.., card)| *card),
        title: None,
        icon: None,
        group: None,
        raised: 0,
        rung: 0,
        minimized: 0,
        fullscreen: false,
        notice: None,
        menu_asked: None,
    }
}

/// Said of a program window placed by an editor other than the shell, which alone places them,
/// or under a host with no desktop.
pub(super) const WINDOW_UNAVAILABLE: &str = "only the desktop shows a program's window";

/// The status of a program window an editor the root embeds placed (`shown-child.window`), as
/// be-host's own shell hears it: worded from the window's (`with_window`), available to the
/// shell alone, under a desktop.
pub(super) fn window_child_status(
    core: &crate::HostCore,
    child: &be_wasm_root_api::ShownChild,
    window: u64,
    by_shell: bool,
) -> HostChildStatus {
    let available = core.desktop && by_shell;
    let mut status = HostChildStatus {
        child: ChildId(child.child),
        available,
        intrinsic: None,
        active: available && child.active,
        interaction: InteractionMode::Preview,
        capabilities: EditorCapabilities::default(),
        resize: ResizeMode::None,
        error: (!available).then(|| WINDOW_UNAVAILABLE.to_owned()),
        menu: Vec::new(),
        creation: None,
        preview: ChildPreview::Hosted,
        dialog: DialogStatus::default(),
        picked: None,
        card: false,
        title: None,
        icon: None,
        group: None,
        raised: 0,
        rung: 0,
        minimized: 0,
        fullscreen: false,
        notice: None,
        menu_asked: None,
    };
    if available && let Some(shown) = window_status(window) {
        crate::compositor::with_window(&mut status, shown);
    }
    status
}

/// Opens the editor `opening` asks for in the app the root's instance runs, which must be the
/// one that edits the block.
pub(super) fn open_editor(
    core: &crate::HostCore,
    opening: &EditorOpening,
    served: &mut HashMap<u64, Forwarded>,
    opened_for: &mut HashMap<(u64, u64), OpenedFor>,
) -> Result<OpenedEditor, String> {
    let (app, editor, shows) = shown_editor(core, &opening.shown)?;
    if opening.app.as_deref() != Some(app.identity.id.as_str()) {
        return Err(format!(
            "{} runs it, which the root's instance does not run",
            app.identity.id
        ));
    }
    let instance = crate::editors::app::next_instance();
    let creation =
        match &shows {
            Shows::Creation(target) => Some(RootCreation {
                target: *target,
                commit: None,
                committed: false,
                progress: None,
                created: crate::editors::app::Inbox::new(&app.identity.id, instance, |event| {
                    match event {
                        be_host_runtime::InstanceEvent::Created(created) => Some(created),
                        _ => None,
                    }
                }),
            }),
            _ => None,
        };
    let mut artifact = None;
    let (role, block) = match shows {
        Shows::Block(opened) => (
            InstanceRole::Editor(
                editor,
                EditorBlock {
                    id: opened.id,
                    block_type: opened.block_type,
                    view_block: opened.view_block,
                },
            ),
            Some(opened.id),
        ),
        Shows::Picker(request) => {
            be_host_runtime::ask_picker(instance, request);
            (InstanceRole::Picker(editor), None)
        }
        Shows::Rename(block, sheet) => (InstanceRole::Rename(editor, block, sheet), None),
        Shows::Export(block, request) => {
            be_host_runtime::ask_export(instance, request);
            (InstanceRole::Export(editor, block), None)
        }
        Shows::Creation(target) => (InstanceRole::Creation(target.editor, target.template), None),
        Shows::ArtifactSettings(block, source, ref exporter, ref data) => {
            artifact = Some((block.id, exporter.clone(), data.clone()));
            (InstanceRole::Artifact(editor, block, source), None)
        }
    };
    opened_for.insert(
        (opening.instance, instance.0),
        OpenedFor {
            app: std::sync::Arc::clone(&app),
            editor,
            block,
            creation,
            artifact: artifact.as_ref().map(|(id, ..)| *id),
        },
    );
    let forwarded = served
        .entry(opening.instance)
        .or_insert_with(|| Forwarded::new(&app));
    // The session's own editor is the shell, as be-host's own UI makes it (`set_shell`): what
    // only the shell hears (be3:desktop's windows, contacts, the screen's input) is its.
    if block.is_some() && block == core.root_session {
        forwarded.set_shell(instance);
    }
    let frames = match &artifact {
        Some((_, exporter, data)) => forwarded.open_artifact(
            instance,
            role,
            exporter,
            data,
            core.client_id,
            core.registry.app_block_types(),
        ),
        None => forwarded.open(
            instance,
            role,
            core.client_id,
            core.registry.app_block_types(),
        ),
    };
    Ok(OpenedEditor {
        editor: instance.0,
        frames,
        name: app.identity.name.clone(),
    })
}

/// The host's notices as `be3:root` carries them, oldest first.
pub(super) fn shown_notices() -> Vec<Notice> {
    notices::shown()
        .into_iter()
        .map(|notice| Notice {
            id: notice.id,
            failure: notice.failure(),
            actions: notice
                .actions()
                .into_iter()
                .map(|(key, label)| NoticeAction {
                    key: key.to_owned(),
                    label: label.to_owned(),
                })
                .collect(),
            message: notice.message,
        })
        .collect()
}

/// Does what the person answered the host's notices with through the root, in order: dismisses
/// a notice answered with no action, copies its report, or opens its link and dismisses it.
pub(super) fn answer_notices(context: &beui::native::Context, answers: Vec<(u64, Option<String>)>) {
    for (id, action) in answers {
        match action.as_deref() {
            // Under `--root` no account is signed in, so no notice is an account's to dismiss
            // on the server too.
            None => {
                notices::dismiss(id);
            }
            Some(notices::COPY_REPORT) => {
                if let Some(report) = notices::report_of(id) {
                    context.copy(Clip::text(report));
                }
            }
            Some(notices::OPEN_LINK) => {
                if let Some(link) = notices::link_of(id)
                    && let Err(error) = be_host_base::platform::open_link(&link)
                {
                    notices::report(error);
                }
                notices::dismiss(id);
            }
            // A key the notice does not offer changes nothing.
            Some(_) => {}
        }
    }
}

/// Puts what `root` copied on the window's clipboard, the last copy staying, and reads the
/// clipboard for each paste it asked for, whose answer `pasted` keeps until the next frame hands
/// it over (`Root::answer_paste`). A copied item that cannot be read is left out, and a copy
/// with none that can be read changes nothing.
pub(super) fn clipboard(
    context: &beui::native::Context,
    root: &mut Root,
    pasted: &Rc<RefCell<Vec<(u64, Option<ClipItem>)>>>,
) {
    for items in root.take_copies() {
        let items: Vec<ClipItem> = items
            .iter()
            .filter_map(|item| be_sdk_api::ops::decode::<ClipItem>(item))
            .collect();
        // A copy none of whose items can be read leaves the clipboard as it was.
        if !items.is_empty() {
            context.copy(Clip::items(items));
        }
    }
    for (id, formats) in root.take_pastes() {
        let pasted = Rc::clone(pasted);
        context.read_clipboard(formats, move |item| pasted.borrow_mut().push((id, item)));
    }
}

/// The fonts be-host bundles, which it tells its apps, as `be3:root` hands them to its root.
pub(super) fn fonts() -> Vec<u8> {
    be_sdk_api::ops::encode(&be_host_runtime::bundled_fonts())
}

pub(super) fn nanos(time: Duration) -> u64 {
    u64::try_from(time.as_nanos()).unwrap_or(u64::MAX)
}
