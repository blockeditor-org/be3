use std::cell::RefCell;
use std::os::fd::OwnedFd;
use std::rc::Rc;
use std::sync::Arc;

use beui::native::Context;
use beui::Event;

use super::Compositor;

#[derive(Default)]
pub(super) struct Clipboard {
    watching: bool,
    copied: Rc<RefCell<Option<String>>>,
    program: Option<String>,
    host_offered: bool,
    asked: Vec<OwnedFd>,
    reading: bool,
    read: Rc<RefCell<Option<Option<String>>>>,
}

impl Compositor {
    pub fn set_primary_selection(&mut self, on: bool) {
        self.server.state.set_primary_selection(on);
    }

    pub(super) fn sync_clipboard(&mut self, context: &Context) {
        let started = self.watch_copies(context);
        let state = &mut self.server.state;
        let clipboard = &mut self.clipboard;
        if started {
            clipboard.host_offered = true;
            state.offer_host_clipboard();
        }
        if state.take_program_copied() {
            clipboard.host_offered = false;
            clipboard.program = None;
        }
        if let Some(text) = state.take_selection_text() {
            clipboard.program = Some(text.clone());
            context.copy_text(text);
        }
        let copied = clipboard.copied.borrow_mut().take();
        if let Some(text) = copied
            && clipboard.program.as_ref() != Some(&text)
        {
            clipboard.program = None;
            clipboard.host_offered = true;
            state.offer_host_clipboard();
        }
        clipboard.asked.extend(state.take_asked_for_host_clipboard());
        let refocused = context.input(|input| input.events.contains(&Event::Focus(true)));
        let answer = clipboard.read.borrow_mut().take();
        if let Some(text) = &answer {
            clipboard.reading = false;
            let text: Arc<str> = text.clone().unwrap_or_default().into();
            for fd in clipboard.asked.drain(..) {
                crate::selection::send_text(fd, text.clone());
            }
        }
        if let Some(Some(text)) = &answer
            && !clipboard.host_offered
            && clipboard.program.as_ref() != Some(text)
        {
            clipboard.program = None;
            clipboard.host_offered = true;
            state.offer_host_clipboard();
        }
        let checking = refocused && !clipboard.host_offered;
        if (checking || !clipboard.asked.is_empty()) && !clipboard.reading {
            clipboard.reading = true;
            let read = clipboard.read.clone();
            context.read_clipboard(move |text| *read.borrow_mut() = Some(text));
        }
    }

    fn watch_copies(&mut self, context: &Context) -> bool {
        let clipboard = &mut self.clipboard;
        if clipboard.watching {
            return false;
        }
        clipboard.watching = true;
        let copied = clipboard.copied.clone();
        let waker = self.waker.clone();
        context.watch_copies(move |text| {
            *copied.borrow_mut() = Some(text.to_owned());
            if let Some(waker) = &waker {
                waker.wake();
            }
        });
        true
    }

    pub(super) fn carry_files(&mut self, context: &Context) {
        let events = context.input(|input| input.events.clone());
        let state = &mut self.server.state;
        let mut dropped = None;
        for event in events {
            match event {
                Event::FileHovered => state.file_hovered(),
                Event::FileHoverCancelled => state.file_drag_cancelled(),
                Event::FileDropped(file) => dropped
                    .get_or_insert_with(Vec::new)
                    .extend(file.path),
                _ => {}
            }
        }
        match dropped {
            Some(files) if !files.is_empty() => state.files_dropped(files),
            Some(_) => state.file_drag_cancelled(),
            None => {}
        }
    }
}
