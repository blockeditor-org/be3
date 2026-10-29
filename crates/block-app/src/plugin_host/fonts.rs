#[cfg(not(target_arch = "wasm32"))]
use std::cell::RefCell;
use std::{collections::HashSet, path::PathBuf};

use block_plugin_api::{FontFace, FontRole, Fonts};

pub(super) fn bundled() -> Fonts {
    let sources = beui_host::native::FontSources::bundled();
    let faces = [
        (FontRole::Text, &sources.proportional),
        (FontRole::Monospace, &sources.monospace),
        (FontRole::Fallback, &sources.fallback),
        (FontRole::Icons, &sources.icons),
    ]
    .into_iter()
    .flat_map(|(role, fonts)| {
        fonts.iter().map(move |font| FontFace {
            role,
            index: font.index,
            data: font.bytes.to_vec(),
        })
    })
    .collect();
    Fonts {
        replace: true,
        faces,
    }
}

pub(super) struct Fallbacks {
    sent: Vec<(PathBuf, u32)>,
    asked: HashSet<char>,
}

impl Fallbacks {
    pub(super) fn new() -> Self {
        Self {
            sent: Vec::new(),
            asked: HashSet::new(),
        }
    }

    pub(super) fn answer(&mut self, missing: &[char]) -> Option<Fonts> {
        let mut faces = Vec::new();
        for character in missing {
            if !self.asked.insert(*character) {
                continue;
            }
            let candidates = candidates(*character);
            if candidates
                .iter()
                .any(|candidate| self.sent.contains(candidate))
            {
                continue;
            }
            let Some((path, index)) = candidates.into_iter().next() else {
                continue;
            };
            let Ok(data) = std::fs::read(&path) else {
                continue;
            };
            self.sent.push((path, index));
            faces.push(FontFace {
                role: FontRole::Fallback,
                index,
                data,
            });
        }
        (!faces.is_empty()).then_some(Fonts {
            replace: false,
            faces,
        })
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn candidates(character: char) -> Vec<(PathBuf, u32)> {
    thread_local! {
        static SYSTEM: RefCell<Option<beui_host::native::SystemFonts>> = const { RefCell::new(None) };
    }
    SYSTEM.with(|system| {
        system
            .borrow_mut()
            .get_or_insert_with(beui_host::native::SystemFonts::new)
            .candidates(character)
    })
}

#[cfg(target_arch = "wasm32")]
fn candidates(_character: char) -> Vec<(PathBuf, u32)> {
    Vec::new()
}
