use std::cell::{Cell, RefCell};

use crate::document::Document;

thread_local! {
    static CURRENT_DOCUMENT: RefCell<Option<Document>> = const { RefCell::new(None) };
    static ACTIVE_DOCUMENT: Cell<*mut Document> = const { Cell::new(std::ptr::null_mut()) };
    static LENT_DOCUMENT: Cell<*mut Document> = const { Cell::new(std::ptr::null_mut()) };
    static LENT_IN_USE: Cell<bool> = const { Cell::new(false) };
}

struct Restore<T: Copy + 'static>(&'static std::thread::LocalKey<Cell<T>>, T);

impl<T: Copy + 'static> Drop for Restore<T> {
    fn drop(&mut self) {
        self.0.with(|cell| cell.set(self.1));
    }
}

pub(crate) fn lend<R>(document: &mut Document, f: impl FnOnce() -> R) -> R {
    let lent: *mut Document = document;
    let _lent = Restore(
        &LENT_DOCUMENT,
        LENT_DOCUMENT.with(|cell| cell.replace(lent)),
    );
    let _in_use = Restore(&LENT_IN_USE, LENT_IN_USE.with(|cell| cell.replace(false)));
    f()
}

struct ActiveDocumentGuard;

impl Drop for ActiveDocumentGuard {
    fn drop(&mut self) {
        ACTIVE_DOCUMENT.with(|active| active.set(std::ptr::null_mut()));
    }
}

pub enum DocumentGuard<'a> {
    Installed(&'a mut Document, Option<::reactive::ZoneGuard>),
    Reentrant,
}

impl Drop for DocumentGuard<'_> {
    fn drop(&mut self) {
        if let DocumentGuard::Installed(document, _) = self {
            let restored = CURRENT_DOCUMENT.with(|cell| cell.borrow_mut().take());
            **document = restored
                .expect("beui::reactive document guard dropped without an installed document");
        }
    }
}

pub fn install(document: &mut Document) -> DocumentGuard<'_> {
    let already_installed = CURRENT_DOCUMENT
        .with(|cell| cell.try_borrow().map(|slot| slot.is_some()))
        .unwrap_or(true);
    if already_installed {
        return DocumentGuard::Reentrant;
    }
    let zone = document.zone();
    let taken = std::mem::take(document);
    CURRENT_DOCUMENT.with(|cell| {
        let previous = cell.borrow_mut().replace(taken);
        assert!(
            previous.is_none(),
            "beui::reactive: a document is already installed on this thread"
        );
    });
    let mut guard = DocumentGuard::Installed(document, None);
    if let DocumentGuard::Installed(_, slot) = &mut guard {
        *slot = Some(::reactive::enter_zone(zone));
    }
    guard
}

pub fn enter<R>(document: &mut Document, f: impl FnOnce() -> R) -> R {
    let (result, installed) = enter_holding_removals(document, f);
    if installed {
        document.drain_removals();
    }
    result
}

pub(crate) fn enter_holding_removals<R>(
    document: &mut Document,
    f: impl FnOnce() -> R,
) -> (R, bool) {
    let context = document.reactive_scope().context();
    let guard = install(document);
    let result = context.run(f);
    let installed = matches!(guard, DocumentGuard::Installed(..));
    drop(guard);
    (result, installed)
}

pub(crate) fn run_in<R>(document: &mut Document, f: impl FnOnce(&mut Document) -> R) -> R {
    enter(document, || with_document(f))
}

pub fn with_reactive_scope<R>(document: &mut Document, f: impl FnOnce() -> R) -> R {
    enter(document, f)
}

pub fn with_document<R>(f: impl FnOnce(&mut Document) -> R) -> R {
    try_with_document(f).expect(
        "with_document called with no document installed; call it from build(), an event \
         handler or one of the document's effects, or wrap it in with_reactive_scope",
    )
}

pub fn try_with_document<R>(f: impl FnOnce(&mut Document) -> R) -> Option<R> {
    let lent = LENT_DOCUMENT.with(Cell::get);
    if !lent.is_null() {
        let reentered = LENT_IN_USE.with(|cell| cell.replace(true));
        debug_assert!(
            !reentered,
            "a build during layout reached the document while it already held it"
        );
        let _in_use = Restore(&LENT_IN_USE, reentered);
        // unsafe(memory)
        return Some(f(unsafe { &mut *lent }));
    }
    CURRENT_DOCUMENT.with(|cell| match cell.try_borrow_mut() {
        Ok(mut slot) => {
            let document = slot.as_mut()?;
            let ptr: *mut Document = document;
            ACTIVE_DOCUMENT.with(|active| active.set(ptr));
            let _guard = ActiveDocumentGuard;
            Some(f(document))
        }
        Err(_) => {
            let ptr = ACTIVE_DOCUMENT.with(Cell::get);
            // unsafe(memory)
            (!ptr.is_null()).then(|| f(unsafe { &mut *ptr }))
        }
    })
}

pub fn layout_text(
    text: &str,
    font: crate::font::FontId,
    layout: crate::font::TextLayout,
) -> Option<crate::font::Galley> {
    try_with_document(|document| document.layout_text(text, font, layout)).flatten()
}

pub fn type_events(events: Vec<crate::input::Input>) {
    with_document(|document| {
        if let Some(context) = document.context() {
            context.type_events(events);
        }
    });
}

pub fn request_typed_paste() {
    with_document(|document| {
        if let Some(context) = document.context() {
            context.request_typed_paste();
        }
    });
}

pub fn toggle_keyboard() {
    with_document(|document| {
        if let Some(context) = document.context() {
            context.toggle_keyboard();
        }
    });
}

pub fn close_keyboard() {
    with_document(|document| {
        if let Some(context) = document.context() {
            context.close_keyboard();
        }
    });
}

pub fn last_pointer() -> Option<crate::input::PointerSample> {
    try_with_document(|document| document.last_pointer).flatten()
}
