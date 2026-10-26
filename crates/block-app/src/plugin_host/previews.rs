use std::{
    cell::RefCell,
    collections::{HashMap, HashSet},
    rc::Rc,
};

use beui::painting::Painting;
use block_plugin_api::{EditorInstanceId, PreviewImage, ScreenId};

use super::backend::Picture;
use super::presenter::Shared;

const NESTED_PAINTINGS: usize = 8;

struct Exported {
    image: PreviewImage,
    picture: Picture,
    screen: ScreenId,
    shared: Rc<RefCell<Shared>>,
}

#[derive(Default)]
struct Previews {
    next: u64,
    latest: HashMap<(String, EditorInstanceId), Exported>,
    offered: HashMap<String, HashSet<u64>>,
    withdrawn: HashMap<String, Vec<u64>>,
}

impl Drop for Exported {
    fn drop(&mut self) {
        super::backend::release(&self.picture);
    }
}

impl Previews {
    fn retire(&mut self, image: u64) {
        for (plugin, offered) in &mut self.offered {
            if offered.remove(&image) {
                self.withdrawn
                    .entry(plugin.clone())
                    .or_default()
                    .push(image);
            }
        }
    }
}

thread_local! {
    static PREVIEWS: RefCell<Previews> = RefCell::new(Previews::default());
}

pub(super) struct Painted {
    pub(super) instance: EditorInstanceId,
    pub(super) screen: ScreenId,
    pub(super) picture: Picture,
    pub(super) size: [u32; 2],
}

pub(super) fn exported(plugin: &str, shared: &Rc<RefCell<Shared>>, painted: Vec<Painted>) {
    if painted.is_empty() {
        return;
    }
    PREVIEWS.with_borrow_mut(|previews| {
        for Painted {
            instance,
            screen,
            picture,
            size: [width, height],
        } in painted
        {
            previews.next += 1;
            let image = PreviewImage {
                image: previews.next,
                width,
                height,
            };
            let exported = Exported {
                image,
                picture,
                screen,
                shared: Rc::clone(shared),
            };
            if let Some(old) = previews
                .latest
                .insert((plugin.to_owned(), instance), exported)
            {
                previews.retire(old.image.image);
            }
        }
    });
}

pub(super) fn keep(plugin: &str, shown: impl Fn(EditorInstanceId, ScreenId) -> bool) {
    PREVIEWS.with_borrow_mut(|previews| {
        let mut gone = Vec::new();
        previews.latest.retain(|(held, instance), exported| {
            let kept = held != plugin || shown(*instance, exported.screen);
            if !kept {
                gone.push(exported.image.image);
            }
            kept
        });
        for image in gone {
            previews.retire(image);
        }
    });
}

pub(super) fn image(plugin: &str, instance: EditorInstanceId) -> Option<PreviewImage> {
    PREVIEWS.with_borrow(|previews| {
        previews
            .latest
            .get(&(plugin.to_owned(), instance))
            .map(|exported| exported.image)
    })
}

pub(super) fn offer(plugin: &str, image: u64) -> Option<Picture> {
    PREVIEWS.with_borrow_mut(|previews| {
        let picture = previews
            .latest
            .values()
            .find(|exported| exported.image.image == image)?
            .picture
            .clone();
        previews
            .offered
            .entry(plugin.to_owned())
            .or_default()
            .insert(image)
            .then_some(picture)
    })
}

pub(super) fn take_withdrawn(plugin: &str) -> Vec<u64> {
    PREVIEWS.with_borrow_mut(|previews| previews.withdrawn.remove(plugin).unwrap_or_default())
}

pub(super) fn forget(plugin: &str) {
    PREVIEWS.with_borrow_mut(|previews| {
        previews.offered.remove(plugin);
        previews.withdrawn.remove(plugin);
        let gone: Vec<u64> = previews
            .latest
            .iter()
            .filter(|((held, _), _)| held == plugin)
            .map(|(_, exported)| exported.image.image)
            .collect();
        previews.latest.retain(|(held, _), _| held != plugin);
        for image in gone {
            previews.retire(image);
        }
    });
}

pub(super) fn painting(painting: Painting) -> Painting {
    nested(painting, NESTED_PAINTINGS)
}

fn nested(painting: Painting, depth: usize) -> Painting {
    painting.with_previews(|image| {
        let (screen, shared) = PREVIEWS.with_borrow(|previews| {
            previews
                .latest
                .values()
                .find(|exported| exported.image.image == image)
                .map(|exported| (exported.screen, Rc::clone(&exported.shared)))
        })?;
        let preview = shared.borrow().paintings.get(&screen).cloned()?;
        Some(match depth {
            0 => preview.with_previews(|_| None),
            _ => nested(preview, depth - 1),
        })
    })
}
