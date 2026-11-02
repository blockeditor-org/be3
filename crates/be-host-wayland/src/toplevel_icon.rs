use std::collections::HashMap;

use beui::Image;
use smithay::delegate_xdg_toplevel_icon;
use smithay::reexports::wayland_server::backend::ObjectId;
use smithay::reexports::wayland_server::protocol::wl_buffer::WlBuffer;
use smithay::reexports::wayland_server::protocol::wl_shm;
use smithay::reexports::wayland_server::protocol::wl_surface::WlSurface;
use smithay::reexports::wayland_server::{DisplayHandle, Resource};
use smithay::wayland::compositor::with_states;
use smithay::wayland::shm::with_buffer_contents;
use smithay::wayland::xdg_toplevel_icon::{
    ToplevelIconCachedState, XdgToplevelIconHandler, XdgToplevelIconManager,
};

use crate::programs::{Environment, IconThemes, load_icon};
use crate::state::{State, WindowId};

pub const ICON_PIXELS: u32 = 64;
const ASKED_SIZES: [i32; 2] = [32, 64];
const MAX_READ_PIXELS: i32 = 512;

#[derive(Clone, Debug, Default, PartialEq)]
struct Key {
    name: Option<String>,
    buffers: Vec<(ObjectId, i32)>,
}

pub(crate) struct ToplevelIcons {
    shown: HashMap<WindowId, (Key, Option<Image>)>,
    themes: Option<IconThemes>,
    named: HashMap<String, Option<Image>>,
}

impl ToplevelIcons {
    pub(crate) fn new(handle: &DisplayHandle) -> Self {
        let mut manager = XdgToplevelIconManager::new::<State>(handle);
        for size in ASKED_SIZES {
            manager.add_icon_size(size);
        }
        Self {
            shown: HashMap::new(),
            themes: None,
            named: HashMap::new(),
        }
    }

    fn named(&mut self, name: &str) -> Option<Image> {
        if let Some(found) = self.named.get(name) {
            return found.clone();
        }
        let themes = self
            .themes
            .get_or_insert_with(|| IconThemes::new(&Environment::from_env()));
        let found = themes
            .find(name, ICON_PIXELS)
            .and_then(|path| load_icon(&path, ICON_PIXELS));
        self.named.insert(name.to_owned(), found.clone());
        found
    }
}

impl State {
    pub fn icon(&self, id: WindowId) -> Option<Image> {
        self.icons.shown.get(&id)?.1.clone()
    }

    pub(crate) fn icon_committed(&mut self, root: &WlSurface) {
        let Some(id) = self.window_of_surface(root) else {
            return;
        };
        let (name, buffers) = with_states(root, |states| {
            let mut cached = states.cached_state.get::<ToplevelIconCachedState>();
            let current = cached.current();
            (
                current.icon_name().map(str::to_owned),
                current.buffers().to_vec(),
            )
        });
        let key = Key {
            name: name.clone(),
            buffers: buffers
                .iter()
                .map(|(buffer, scale)| (buffer.id(), *scale))
                .collect(),
        };
        let known = self.icons.shown.get(&id).map(|(known, _)| known);
        if known == Some(&key) || (known.is_none() && key == Key::default()) {
            return;
        }
        let named = name.as_deref().and_then(|name| self.icons.named(name));
        let image = named.or_else(|| pixels(&buffers));
        self.icons.shown.insert(id, (key, image));
        self.changed(id);
    }

    pub(crate) fn forget_icon(&mut self, id: WindowId) {
        self.icons.shown.remove(&id);
    }
}

fn pixels(buffers: &[(WlBuffer, i32)]) -> Option<Image> {
    let sized: Vec<(&WlBuffer, i32)> = buffers
        .iter()
        .filter_map(|(buffer, _)| {
            let width = with_buffer_contents(buffer, |_, _, data| data.width).ok()?;
            (width <= MAX_READ_PIXELS).then_some((buffer, width))
        })
        .collect();
    let (buffer, _) = sized
        .iter()
        .filter(|(_, width)| *width as u32 >= ICON_PIXELS)
        .min_by_key(|(_, width)| *width)
        .or_else(|| sized.iter().max_by_key(|(_, width)| *width))?;
    let image = read(buffer)?;
    match image.width() > ICON_PIXELS {
        true => Some(shrink(&image)),
        false => Some(image),
    }
}

fn read(buffer: &WlBuffer) -> Option<Image> {
    with_buffer_contents(buffer, |pointer, length, data| {
        let opaque = match data.format {
            wl_shm::Format::Argb8888 => false,
            wl_shm::Format::Xrgb8888 => true,
            _ => return None,
        };
        if data.width <= 0
            || data.height <= 0
            || data.width > MAX_READ_PIXELS
            || data.height > MAX_READ_PIXELS
        {
            return None;
        }
        let (width, height) = (data.width as usize, data.height as usize);
        let (offset, stride) = (data.offset.max(0) as usize, data.stride as usize);
        if stride < width * 4 || offset + stride * (height - 1) + width * 4 > length {
            return None;
        }
        let needed = stride * (height - 1) + width * 4;
        let mut bytes = vec![0u8; needed];
        // unsafe(ffi)
        unsafe {
            std::ptr::copy_nonoverlapping(pointer.add(offset), bytes.as_mut_ptr(), needed);
        }
        let mut rgba = Vec::with_capacity(width * height * 4);
        for row in 0..height {
            for pixel in bytes[row * stride..row * stride + width * 4]
                .as_chunks::<4>()
                .0
            {
                let [blue, green, red, alpha] = [pixel[0], pixel[1], pixel[2], pixel[3]];
                let alpha = if opaque { 0xff } else { alpha };
                let straight = |channel: u8| match alpha {
                    0 => 0,
                    _ => ((u32::from(channel) * 255 + u32::from(alpha) / 2) / u32::from(alpha))
                        .min(255) as u8,
                };
                rgba.extend([straight(red), straight(green), straight(blue), alpha]);
            }
        }
        Some(Image::from_rgba(width as u32, height as u32, rgba))
    })
    .ok()
    .flatten()
}

fn shrink(image: &Image) -> Image {
    let source = image::RgbaImage::from_raw(image.width(), image.height(), image.pixels().to_vec())
        .expect("an image carries four bytes per pixel");
    let height = (image.height() * ICON_PIXELS / image.width()).max(1);
    let shrunk = image::imageops::resize(
        &source,
        ICON_PIXELS,
        height,
        image::imageops::FilterType::Triangle,
    );
    Image::from_rgba(ICON_PIXELS, height, shrunk.into_raw())
}

impl XdgToplevelIconHandler for State {}

delegate_xdg_toplevel_icon!(State);
