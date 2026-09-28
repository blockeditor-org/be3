use block_plugin_api::{FontFace, FontRole, Fonts};

pub(super) fn bundled() -> Fonts {
    let sources = beui::FontSources::bundled();
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

#[cfg(not(target_arch = "wasm32"))]
pub(super) use native::Fallbacks;

#[cfg(not(target_arch = "wasm32"))]
mod native {
    use std::cell::RefCell;
    use std::path::PathBuf;

    use block_plugin_api::{FontFace, FontRole, Fonts};

    thread_local! {
        static SYSTEM: RefCell<Option<beui::SystemFonts>> = const { RefCell::new(None) };
    }

    #[derive(Default)]
    pub(in crate::plugin_host) struct Fallbacks {
        sent: Vec<(PathBuf, u32)>,
    }

    impl Fallbacks {
        pub(in crate::plugin_host) fn answer(&mut self, missing: &[char]) -> Option<Fonts> {
            let mut faces = Vec::new();
            for character in missing {
                let candidates = SYSTEM.with(|system| {
                    system
                        .borrow_mut()
                        .get_or_insert_with(beui::SystemFonts::new)
                        .candidates(*character)
                });
                if candidates.iter().any(|candidate| self.sent.contains(candidate)) {
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
}

#[cfg(target_arch = "wasm32")]
#[derive(Default)]
pub(super) struct Fallbacks;

#[cfg(target_arch = "wasm32")]
impl Fallbacks {
    pub(super) fn answer(&mut self, _missing: &[char]) -> Option<Fonts> {
        None
    }
}
