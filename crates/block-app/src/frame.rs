use std::cell::RefCell;

use beui::native::{FilePickId, FileSaveId};
use block_app_base::host::{self, Frame, PickSlot, SaveSlot};

thread_local! {
    static WAITING: RefCell<Waiting> = RefCell::new(Waiting::default());
}

#[derive(Default)]
struct Waiting {
    picks: Vec<(FilePickId, PickSlot)>,
    saves: Vec<(FileSaveId, SaveSlot)>,
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
    for (deliver, pick) in picked {
        deliver(pick);
    }
    for (deliver, save) in saved {
        deliver(save);
    }
    block_app_stack::hold_clock(context.clock_stopped().then(|| context.now()));
    let mut frame = Frame {
        now: Some(context.now()),
        wall: Some(beui::reactive::wall_clock()),
        seen: context.seen(),
        clock_stopped: context.clock_stopped(),
        ..Frame::default()
    };
    frame.preferences = context.preferences();
    let layers = context.preference_layers();
    block_app_base::watches::publish::<block_plugin_api::PreferenceSourcesWatch>(
        &block_plugin_api::HostPreferenceSources {
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
    if let Some(text) = output.copied {
        context.copy_text(text);
    }
    if let Some(png) = output.copied_image {
        context.copy_image(png);
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
}
