use std::cell::RefCell;
use std::collections::BTreeMap;
use std::os::fd::OwnedFd;
use std::rc::Rc;
use std::sync::Arc;

use beui::native::Context;
use beui::{Clip, ClipItem, Event};

use super::Compositor;

/// Bridges the host's clipboard and the programs'. A program's copy is read at
/// once, its text and image, and copied in the host, so it outlives the
/// program. A host copy is offered to programs under the MIME types of every
/// format it holds; a program asking for one reads it from the host's clip.
#[derive(Default)]
pub(super) struct Clipboard {
    watching: bool,
    copied: Rc<RefCell<Option<Vec<String>>>>,
    program: Option<Vec<ClipItem>>,
    host_offered: bool,
    asked: Vec<(String, OwnedFd)>,
    reading: BTreeMap<String, Rc<RefCell<Option<Option<ClipItem>>>>>,
    checking: Option<Rc<RefCell<Option<Option<ClipItem>>>>>,
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
            state.offer_host_clipboard(&[beui::clipboard::formats::TEXT.to_owned()]);
        }
        if state.take_program_copied() {
            clipboard.host_offered = false;
            clipboard.program = None;
        }
        if let Some(items) = state.take_selection_items() {
            clipboard.program = Some(items.clone());
            context.copy(Clip::items(items));
        }
        let copied = clipboard.copied.borrow_mut().take();
        if let Some(formats) = copied {
            let echoed = clipboard
                .program
                .as_ref()
                .is_some_and(|items| items.iter().map(|item| &item.format).eq(formats.iter()));
            if !echoed {
                clipboard.program = None;
                clipboard.host_offered = true;
                state.offer_host_clipboard(&formats);
            }
        }
        clipboard
            .asked
            .extend(state.take_asked_for_host_clipboard());
        let refocused = context.input(|input| input.events.contains(&Event::Focus(true)));
        clipboard.answer_asked();
        clipboard.check_host(context, refocused, state);
        for format in clipboard
            .asked
            .iter()
            .map(|(format, _)| format.clone())
            .collect::<Vec<_>>()
        {
            if clipboard.reading.contains_key(&format) {
                continue;
            }
            let read = Rc::new(RefCell::new(None));
            clipboard.reading.insert(format.clone(), read.clone());
            context.read_clipboard(vec![format], move |item| *read.borrow_mut() = Some(item));
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
        context.watch_copies(move |clip| {
            *copied.borrow_mut() = Some(clip.formats().map(str::to_owned).collect());
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
                Event::FileDropped(file) => dropped.get_or_insert_with(Vec::new).extend(file.path),
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

impl Clipboard {
    /// Writes each format the host's clip answered to the pipes that asked
    /// for it; nothing when the clip does not hold it.
    fn answer_asked(&mut self) {
        let answered = self
            .reading
            .iter()
            .filter_map(|(format, read)| Some((format.clone(), read.borrow_mut().take()?)))
            .collect::<Vec<_>>();
        for (format, item) in answered {
            self.reading.remove(&format);
            let data: Arc<[u8]> = item.map(|item| item.data).unwrap_or_default().into();
            for (_, fd) in self.asked.extract_if(.., |(asked, _)| *asked == format) {
                crate::selection::send_data(fd, data.clone());
            }
        }
    }

    /// When the window is focused again and a program owns the selection, the
    /// host's own clipboard may have changed meanwhile (a program outside the
    /// compositor copied): its text, when it differs from the program's, is
    /// offered instead.
    fn check_host(&mut self, context: &Context, refocused: bool, state: &mut crate::state::State) {
        let answered = self
            .checking
            .as_ref()
            .and_then(|read| read.borrow_mut().take());
        if let Some(answer) = answered {
            self.checking = None;
            let program_text = self
                .program
                .as_ref()
                .and_then(|items| items.iter().find_map(ClipItem::as_text));
            if let Some(text) = answer.and_then(|item| item.as_text())
                && !self.host_offered
                && program_text.as_ref() != Some(&text)
            {
                self.program = None;
                self.host_offered = true;
                state.offer_host_clipboard(&[beui::clipboard::formats::TEXT.to_owned()]);
            }
        }
        if refocused && !self.host_offered && self.checking.is_none() {
            let read = Rc::new(RefCell::new(None));
            self.checking = Some(read.clone());
            context.read_clipboard(
                vec![beui::clipboard::formats::TEXT.to_owned()],
                move |item| *read.borrow_mut() = Some(item),
            );
        }
    }
}
