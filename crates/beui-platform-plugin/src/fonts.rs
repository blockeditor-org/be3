use std::cell::RefCell;

use beui_core::context::Context;
use beui_font_freetype::{FontBytes, FontData, FontLibrary, FontSources, FreetypeFonts};
use block_editor_plugin::fonts::{HostFont, host_fonts, report_missing, watch_fonts};
use block_plugin_api::FontRole;

thread_local! {
    static LIBRARY: RefCell<Option<FontLibrary>> = const { RefCell::new(None) };
}

fn fonts() -> FontLibrary {
    if let Some(library) = LIBRARY.with(|library| library.borrow().clone()) {
        return library;
    }
    let library = FontLibrary::new(sources(&host_fonts())).with_fallback(|character| {
        report_missing([character]);
        None
    });
    let watched = library.clone();
    watch_fonts(move |added, replaced| match replaced {
        true => watched.replace(sources(added)),
        false => watched.add_fallback(added.iter().map(data).collect()),
    });
    LIBRARY.with(|held| *held.borrow_mut() = Some(library.clone()));
    library
}

pub(crate) fn use_bundled() {
    LIBRARY.with(|held| *held.borrow_mut() = Some(FontLibrary::bundled()));
}

pub(crate) fn context() -> Context {
    Context::new(FreetypeFonts::new(fonts()))
}

fn sources(faces: &[HostFont]) -> FontSources {
    let mut sources = FontSources::default();
    for face in faces {
        let list = match face.role {
            FontRole::Text => &mut sources.proportional,
            FontRole::Monospace => &mut sources.monospace,
            FontRole::Fallback => &mut sources.fallback,
            FontRole::Icons => &mut sources.icons,
        };
        list.push(data(face));
    }
    sources
}

fn data(face: &HostFont) -> FontData {
    FontData::new(FontBytes::Shared(face.data.clone()), face.index)
}
