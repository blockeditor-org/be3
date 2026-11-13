use std::cell::RefCell;

use be_host_base::host::{self, Frame, PickSlot, SaveSlot};
use beui::native::{FilePickId, FileSaveId};

thread_local! {
    static WAITING: RefCell<Waiting> = RefCell::new(Waiting::default());
}

#[derive(Default)]
struct Waiting {
    picks: Vec<(FilePickId, PickSlot)>,
    saves: Vec<(FileSaveId, SaveSlot)>,
    renders: Vec<Render>,
    /// Hears the instances' answers to `renders`, while any wait.
    rendered: Option<host::Mailbox>,
    exports: Vec<Export>,
}

/// The files of copied blocks a paste asked for, read by the blocks' types.
struct Export {
    read: std::sync::mpsc::Receiver<Option<beui::clipboard::ClipFile>>,
    left: usize,
    files: Vec<beui::clipboard::ClipFile>,
    answer: Box<dyn FnOnce(Option<Vec<u8>>)>,
}

/// A representation of an app's copy that a paste asked the app to render.
struct Render {
    app_id: String,
    instance: be_sdk_api::EditorInstanceId,
    request: u64,
    answer: Box<dyn FnOnce(Option<Vec<u8>>)>,
}

/// The clip an app copied: its ready items, and the rest rendered by the
/// instance when a paste asks, which answer nothing once it stopped.
fn app_clip(copied: host::CopiedClip) -> beui::Clip {
    let host::CopiedClip {
        app_id,
        instance,
        serial,
        items,
    } = copied;
    let blocks = items
        .iter()
        .filter(|item| item.format == beui::clipboard::formats::BLOCKS)
        .find_map(|item| {
            let item = beui::ClipItem::new(item.format.clone(), item.data.clone()?);
            item.decode::<beui::clipboard::Blocks>()
        });
    let clip = items
        .into_iter()
        .fold(beui::Clip::new(), |clip, item| match item.data {
            Some(data) => clip.with(beui::ClipItem::new(item.format, data)),
            None => {
                let app_id = app_id.clone();
                let format = item.format.clone();
                clip.with_lazy(
                    item.format,
                    move |answer| match be_host_runtime::render_clip(
                        &app_id,
                        instance,
                        serial,
                        format.clone(),
                    ) {
                        Some(request) => WAITING.with(|waiting| {
                            let mut waiting = waiting.borrow_mut();
                            waiting.renders.push(Render {
                                app_id: app_id.clone(),
                                instance,
                                request,
                                answer,
                            });
                            if waiting.rendered.is_none() {
                                let (mailbox, post) = host::mailbox(rendered);
                                be_host_runtime::listen(post);
                                waiting.rendered = Some(mailbox);
                            }
                        }),
                        None => answer(None),
                    },
                )
            }
        });
    match blocks {
        Some(blocks) if !clip.holds(beui::clipboard::formats::FILES) => {
            let ids = blocks
                .blocks
                .iter()
                .map(|block| block.id)
                .collect::<Vec<_>>();
            clip.with_lazy(beui::clipboard::formats::FILES, move |answer| {
                export_files(&ids, answer)
            })
        }
        _ => clip,
    }
}

/// Reads the plain file each block's type writes it as, for a paste of the
/// blocks' files; a block the person cannot view, or without a file form, is
/// left out.
fn export_files(blocks: &[uuid::Uuid], answer: Box<dyn FnOnce(Option<Vec<u8>>)>) {
    let (send, read) = std::sync::mpsc::channel();
    let mut left = 0;
    for block in blocks {
        if !be_host_stack::access(*block).can_view() {
            continue;
        }
        left += 1;
        let send = send.clone();
        be_host_stack::plain_file(*block, move |file| {
            let file = file.ok().map(|file| beui::clipboard::ClipFile {
                name: file.name.unwrap_or_default(),
                data: file.bytes,
            });
            let _ = send.send(file);
        });
    }
    if left == 0 {
        return answer(None);
    }
    WAITING.with(|waiting| {
        waiting.borrow_mut().exports.push(Export {
            read,
            left,
            files: Vec::new(),
            answer,
        })
    });
}

/// Answers the paste waiting for what an instance rendered.
fn rendered(heard: be_host_runtime::Heard) {
    let be_host_runtime::InstanceEvent::Rendered { request, data } = heard.event else {
        return;
    };
    let answered = WAITING.with(|waiting| {
        let mut waiting = waiting.borrow_mut();
        let at = waiting.renders.iter().position(|render| {
            render.request == request && render.instance == heard.instance && render.app_id == heard.app_id
        })?;
        let render = waiting.renders.remove(at);
        if waiting.renders.is_empty() {
            waiting.rendered = None;
        }
        Some(render.answer)
    });
    if let Some(answer) = answered {
        answer(data);
    }
}

pub(crate) fn begin(context: &beui::native::Context) {
    let mut picked = Vec::new();
    let mut saved = Vec::new();
    WAITING.with(|waiting| {
        let mut waiting = waiting.borrow_mut();
        waiting
            .picks
            .retain_mut(|(id, slot)| match context.take_file_pick(*id) {
                Some(pick) => {
                    picked.extend(slot.take().map(|deliver| (deliver, pick)));
                    false
                }
                None => true,
            });
        for (id, deliver) in std::mem::take(&mut waiting.saves) {
            match context.take_file_save(id) {
                Some(save) => saved.push((deliver, save)),
                None => waiting.saves.push((id, deliver)),
            }
        }
    });
    let exported = WAITING.with(|waiting| {
        let mut waiting = waiting.borrow_mut();
        for export in &mut waiting.exports {
            while let Ok(file) = export.read.try_recv() {
                export.left -= 1;
                export.files.extend(file);
            }
        }
        let (done, left) = std::mem::take(&mut waiting.exports)
            .into_iter()
            .partition::<Vec<_>, _>(|export| export.left == 0);
        waiting.exports = left;
        done
    });
    for export in exported {
        let files = beui::clipboard::Files {
            files: export.files,
        };
        (export.answer)(
            (!files.files.is_empty()).then(|| beui::clipboard::ClipFormat::encode(&files).data),
        );
    }
    for (deliver, pick) in picked {
        deliver(pick);
    }
    for (deliver, save) in saved {
        deliver(save);
    }
    be_host_stack::hold_clock(context.clock_stopped().then(|| context.now()));
    be_host_stack::pin_wall_clock(
        beui::reactive::wall_clock_is_pinned().then(beui::reactive::wall_clock),
    );
    let mut frame = Frame {
        now: Some(context.now()),
        wall: Some(beui::reactive::wall_clock()),
        seen: context.seen(),
        clock_stopped: context.clock_stopped(),
        ..Frame::default()
    };
    frame.preferences = context.preferences();
    let layers = context.preference_layers();
    be_host_base::watches::publish::<be_sdk_api::PreferenceSourcesWatch>(
        &be_sdk_api::HostPreferenceSources {
            system: layers.layer(beui::PreferenceSource::System),
            developer: layers.layer(beui::PreferenceSource::Developer),
        },
    );
    frame.screen_scale = context.screen_scale();
    frame.pixels_per_point = context.pixels_per_point() * frame.screen_scale;
    frame.opens_windows = context.opens_windows();
    host::start(frame);
}

pub(crate) fn end(context: &beui::native::Context) {
    let output = host::finish();
    if let Some(fullscreen) = output.fullscreen {
        context.set_fullscreen(fullscreen);
    }
    if let Some(copied) = output.copied {
        context.copy(app_clip(copied));
    }
    for (filter, slot) in output.file_picks {
        let id = context.pick_file(filter);
        WAITING.with(|waiting| waiting.borrow_mut().picks.push((id, slot)));
    }
    for (file, deliver) in host::take_unsent_saves() {
        let id = context.save_file(file);
        WAITING.with(|waiting| waiting.borrow_mut().saves.push((id, deliver)));
    }
    if output.paste {
        context.request_paste();
    }
    if let Some(grab) = output.grab {
        context.set_pointer_locked(grab);
    }
    if output.repaint {
        context.request_repaint();
    }
    if let Some(delay) = output.repaint_after {
        context.request_repaint_after(delay);
    }
    if output.animating {
        context.note_animating();
    }
    if output.close {
        context.close_window();
    }
    if output.raise {
        context.raise_window();
    }
}
