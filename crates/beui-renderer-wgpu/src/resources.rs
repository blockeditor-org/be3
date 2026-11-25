use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use be_lib_stable_hash::HashMap;

use beui_core::font::{GlyphId, GlyphImage};
use beui_types::{Image, ImageId};

use crate::Atlas;

/// Whether a device is lost: set by the device's one lost callback, which whoever opened the
/// device holds (`Lost::watch`), and read by every renderer on it, which stops drawing while it is
/// set (Vello's buffer uploads panic on a lost device). The platform opens a new device and new
/// renderers on it.
#[derive(Clone, Default, Debug)]
pub struct Lost(Arc<AtomicBool>);

impl Lost {
    /// Takes `device`'s lost callback, which sets the flag this returns.
    pub fn watch(device: &wgpu::Device) -> Self {
        let lost = Self::default();
        let flag = lost.clone();
        device.set_device_lost_callback(move |reason, message| {
            if reason != wgpu::DeviceLostReason::Destroyed {
                eprintln!("beui: the GPU was lost: {message}");
            }
            flag.set();
        });
        lost
    }

    /// Makes `device`'s uncaptured errors panic, as wgpu's default does, until this is set:
    /// after a loss every call fails, and the platform recovers rather than panicking.
    pub fn panic_on_errors(&self, device: &wgpu::Device) {
        let lost = self.clone();
        device.on_uncaptured_error(Arc::new(move |error| {
            if !lost.is_lost() {
                panic!("wgpu error: {error}");
            }
        }));
    }

    pub fn set(&self) {
        self.0.store(true, Ordering::SeqCst);
    }

    pub fn is_lost(&self) -> bool {
        self.0.load(Ordering::SeqCst)
    }
}

#[derive(Clone)]
/// What every renderer on one device and surface format shares: the glyph atlases and
/// uploaded images (kept while any renderer drew them last frame). Give every window or panel
/// on a device a `Renderer` over the same `Resources`. A full atlas is replaced rather than
/// cleared and a renderer draws with what it was prepared against, so renderers sharing it may
/// prepare, render and submit in any order.
pub struct Resources(pub(crate) Rc<RefCell<Shared>>);

pub(crate) struct Shared {
    pub(crate) device: wgpu::Device,
    pub(crate) format: wgpu::TextureFormat,
    pub(crate) atlas: Atlas,
    pub(crate) colors: Option<Atlas>,
    pub(crate) pictures: HashMap<ImageId, Picture>,
    pub(crate) epoch: u64,
    pub(crate) lost: Lost,
}

/// An uploaded image: premultiplied RGBA8, as Vello samples it, with a view of each level of
/// its mip chain.
pub(crate) struct Picture {
    pub(crate) levels: Vec<(wgpu::TextureView, [u32; 2])>,
    users: usize,
}

impl Resources {
    /// Resources on `device`, which `lost` says the loss of.
    pub fn new(device: &wgpu::Device, format: wgpu::TextureFormat, lost: &Lost) -> Self {
        Self(Rc::new(RefCell::new(Shared {
            device: device.clone(),
            lost: lost.clone(),
            format,
            atlas: Atlas::new(device, "beui glyph atlas"),
            colors: None,
            pictures: HashMap::default(),
            epoch: 0,
        })))
    }

    /// The resources this thread last made for `device` and `format`, or new ones: what every
    /// target a guest draws on one device shares (an app module's screens, a root's own
    /// surfaces), so each glyph is uploaded once.
    pub fn shared(device: &wgpu::Device, format: wgpu::TextureFormat, lost: &Lost) -> Self {
        thread_local! {
            static HELD: RefCell<Option<(wgpu::Device, Resources)>> = const { RefCell::new(None) };
        }
        HELD.with(|held| {
            let mut held = held.borrow_mut();
            match &*held {
                Some((made_on, resources)) if made_on == device && resources.format() == format => {
                    resources.clone()
                }
                _ => {
                    let resources = Self::new(device, format, lost);
                    *held = Some((device.clone(), resources.clone()));
                    resources
                }
            }
        })
    }

    pub fn format(&self) -> wgpu::TextureFormat {
        self.0.borrow().format
    }

    /// Whether their device is lost.
    pub fn lost(&self) -> bool {
        self.0.borrow().lost.is_lost()
    }

    pub fn holds_colour_glyphs(&self) -> bool {
        self.0.borrow().colors.is_some()
    }

    pub fn pictures(&self) -> usize {
        self.0.borrow().pictures.len()
    }

    pub fn same(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
}

impl Shared {
    pub(crate) fn full(&self) -> bool {
        self.atlas.full || self.colors.as_ref().is_some_and(|colors| colors.full)
    }

    pub(crate) fn renew(&mut self) {
        self.atlas = Atlas::new(&self.device, "beui glyph atlas");
        if self.colors.is_some() {
            self.colors = Some(Atlas::new(&self.device, "beui colour glyph atlas"));
        }
        self.epoch += 1;
    }

    /// Where the glyph lies in its atlas, in texels, and whether it is a colour glyph.
    pub(crate) fn glyph(
        &mut self,
        queue: &wgpu::Queue,
        id: GlyphId,
        image: &GlyphImage,
    ) -> Option<[u32; 4]> {
        if !image.color {
            return self.atlas.insert(queue, id, image);
        }
        let device = &self.device;
        self.colors
            .get_or_insert_with(|| Atlas::new(device, "beui colour glyph atlas"))
            .insert(queue, id, image)
    }

    pub(crate) fn upload(&mut self, queue: &wgpu::Queue, image: &Image) {
        if self.pictures.contains_key(&image.id()) {
            return;
        }
        let levels = mip_levels(image.width(), image.height());
        let texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("beui image"),
            size: wgpu::Extent3d {
                width: image.width(),
                height: image.height(),
                depth_or_array_layers: 1,
            },
            mip_level_count: levels,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let mut views = Vec::with_capacity(levels as usize);
        let mut level = (image.width(), image.height(), None::<Vec<u8>>);
        for mip_level in 0..levels {
            let (width, height, ref shrunk) = level;
            let pixels = shrunk.as_deref().unwrap_or(image.pixels());
            queue.write_texture(
                wgpu::TexelCopyTextureInfo {
                    texture: &texture,
                    mip_level,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                &premultiplied(pixels),
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(width * 4),
                    rows_per_image: Some(height),
                },
                wgpu::Extent3d {
                    width,
                    height,
                    depth_or_array_layers: 1,
                },
            );
            views.push((
                texture.create_view(&wgpu::TextureViewDescriptor {
                    base_mip_level: mip_level,
                    mip_level_count: Some(1),
                    ..Default::default()
                }),
                [width, height],
            ));
            if mip_level + 1 < levels {
                level = halved(pixels, width, height);
            }
        }
        self.pictures.insert(
            image.id(),
            Picture {
                levels: views,
                users: 0,
            },
        );
    }

    pub(crate) fn hold(&mut self, id: ImageId) {
        if let Some(picture) = self.pictures.get_mut(&id) {
            picture.users += 1;
        }
    }

    pub(crate) fn release(&mut self, id: ImageId) {
        let Some(picture) = self.pictures.get_mut(&id) else {
            return;
        };
        picture.users = picture.users.saturating_sub(1);
        if picture.users == 0 {
            self.pictures.remove(&id);
        }
    }
}

fn mip_levels(width: u32, height: u32) -> u32 {
    u32::BITS - width.max(height).max(1).leading_zeros()
}

/// Straight-alpha RGBA8 texels premultiplied, as Vello samples them.
pub(crate) fn premultiplied(pixels: &[u8]) -> Vec<u8> {
    pixels
        .chunks_exact(4)
        .flat_map(|texel| {
            let alpha = u32::from(texel[3]);
            let scaled = |channel: u8| ((u32::from(channel) * alpha + 127) / 255) as u8;
            [scaled(texel[0]), scaled(texel[1]), scaled(texel[2]), texel[3]]
        })
        .collect()
}

fn halved(pixels: &[u8], width: u32, height: u32) -> (u32, u32, Option<Vec<u8>>) {
    let (to_width, to_height) = ((width / 2).max(1), (height / 2).max(1));
    let mut out = Vec::with_capacity(to_width as usize * to_height as usize * 4);
    let texel = |x: u32, y: u32| {
        let at = (y.min(height - 1) as usize * width as usize + x.min(width - 1) as usize) * 4;
        &pixels[at..at + 4]
    };
    for y in 0..to_height {
        for x in 0..to_width {
            let mut sums = [0u32; 4];
            for (dx, dy) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
                let texel = texel(x * 2 + dx, y * 2 + dy);
                let alpha = u32::from(texel[3]);
                for channel in 0..3 {
                    sums[channel] += u32::from(texel[channel]) * alpha;
                }
                sums[3] += alpha;
            }
            let colour = |sum: u32| match sums[3] {
                0 => 0,
                alpha => ((sum + alpha / 2) / alpha) as u8,
            };
            out.extend([
                colour(sums[0]),
                colour(sums[1]),
                colour(sums[2]),
                ((sums[3] + 2) / 4) as u8,
            ]);
        }
    }
    (to_width, to_height, Some(out))
}
