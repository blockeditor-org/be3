use std::cell::{Cell, RefCell};

use crate::document::Document;

thread_local! {
    static CURRENT_DOCUMENT: RefCell<Option<Document>> = const { RefCell::new(None) };
    static ACTIVE_DOCUMENT: Cell<*mut Document> = const { Cell::new(std::ptr::null_mut()) };
}

struct ActiveDocumentGuard;

impl Drop for ActiveDocumentGuard {
    fn drop(&mut self) {
        ACTIVE_DOCUMENT.with(|active| active.set(std::ptr::null_mut()));
    }
}

pub enum DocumentGuard<'a> {
    Installed(&'a mut Document),
    Reentrant,
}

impl Drop for DocumentGuard<'_> {
    fn drop(&mut self) {
        if let DocumentGuard::Installed(document) = self {
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
    let taken = std::mem::take(document);
    CURRENT_DOCUMENT.with(|cell| {
        let previous = cell.borrow_mut().replace(taken);
        assert!(
            previous.is_none(),
            "beui::reactive: a document is already installed on this thread"
        );
    });
    DocumentGuard::Installed(document)
}

pub fn enter<R>(document: &mut Document, f: impl FnOnce() -> R) -> R {
    let context = document.reactive_scope().context();
    let _guard = install(document);
    context.run(f)
}

pub fn with_reactive_scope<R>(document: &mut Document, f: impl FnOnce() -> R) -> R {
    enter(document, f)
}

pub fn with_document<R>(f: impl FnOnce(&mut Document) -> R) -> R {
    try_with_document(f).expect(
        "beui::reactive binding used without an active document; \
         call it from inside build(), or from inside event dispatch",
    )
}

pub fn try_with_document<R>(f: impl FnOnce(&mut Document) -> R) -> Option<R> {
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
            (!ptr.is_null()).then(|| f(unsafe { &mut *ptr }))
        }
    })
}
