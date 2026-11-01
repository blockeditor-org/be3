use std::cell::RefCell;
use std::os::fd::OwnedFd;
use std::rc::Rc;

use be_dmabuf::{SyncFiles, Vulkan};
use beui::Vec2;
use beui::native::texture_quad::{Bound, Pipelines, Quad};

use crate::rotation::Rotation;

pub struct CursorImage {
    pub texture: wgpu::Texture,
    pub size: Vec2,
    pub hotspot: Vec2,
    pub opaque: bool,
    pub revision: u64,
}

#[derive(Clone, Default)]
pub struct SoftwareCursor(Rc<RefCell<Option<CursorImage>>>);

impl SoftwareCursor {
    pub fn set(&self, image: Option<CursorImage>) {
        *self.0.borrow_mut() = image;
    }

    pub fn shown(&self) -> bool {
        self.0.borrow().is_some()
    }

    pub(crate) fn with<R>(&self, read: impl FnOnce(Option<&CursorImage>) -> R) -> R {
        read(self.0.borrow().as_ref())
    }
}

pub struct Sprite {
    bound: Bound,
    opaque: bool,
}

pub enum Submitted {
    Fence(Option<OwnedFd>),
    Index(wgpu::SubmissionIndex),
}

pub struct Gpu {
    sync: RefCell<Option<SyncFiles>>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    vulkan: Option<Vulkan>,
    quads: Pipelines,
    format: wgpu::TextureFormat,
}

impl Gpu {
    pub fn new(device: wgpu::Device, queue: wgpu::Queue, format: wgpu::TextureFormat) -> Self {
        let vulkan = Vulkan::of(&device);
        let sync = vulkan.as_ref().and_then(Vulkan::sync_files);
        if sync.is_none() {
            eprintln!("beui: the GPU cannot export fences, so each frame is waited for");
        }
        Self {
            sync: RefCell::new(sync),
            quads: Pipelines::new(&device),
            device,
            queue,
            vulkan,
            format,
        }
    }

    pub fn device(&self) -> &wgpu::Device {
        &self.device
    }

    pub fn queue(&self) -> &wgpu::Queue {
        &self.queue
    }

    pub fn vulkan(&self) -> Option<&Vulkan> {
        self.vulkan.as_ref()
    }

    pub fn submit(&self, commands: wgpu::CommandBuffer) -> Submitted {
        let mut sync = self.sync.borrow_mut();
        let Some(files) = sync.as_ref() else {
            return Submitted::Index(self.queue.submit([commands]));
        };
        match files.submit(&self.queue, commands) {
            Ok(fence) => Submitted::Fence(fence),
            Err(error) => {
                eprintln!("beui: {error}, so each frame is waited for from now on");
                *sync = None;
                Submitted::Index(self.queue.submit([]))
            }
        }
    }

    pub fn rgba(&self, width: u32, height: u32, pixels: &[u8]) -> Sprite {
        let size = wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        };
        let texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("sprite"),
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        self.queue.write_texture(
            texture.as_image_copy(),
            pixels,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(width * 4),
                rows_per_image: Some(height),
            },
            size,
        );
        self.sprite(&texture, false)
    }

    pub fn sprite(&self, texture: &wgpu::Texture, opaque: bool) -> Sprite {
        Sprite {
            bound: self.quads.bind(texture),
            opaque,
        }
    }

    pub fn paint_sprite(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        target: &wgpu::TextureView,
        target_size: (u32, u32),
        rect: [f32; 4],
        sprite: &Sprite,
        rotation: Rotation,
    ) {
        let [left, top, right, bottom] = rect;
        let (center_x, center_y) = ((left + right) * 0.5, (top + bottom) * 0.5);
        let (half_x, half_y) = match rotation.sideways() {
            true => ((bottom - top) * 0.5, (right - left) * 0.5),
            false => ((right - left) * 0.5, (bottom - top) * 0.5),
        };
        let quad = Quad {
            rect: [
                center_x - half_x,
                center_y - half_y,
                center_x + half_x,
                center_y + half_y,
            ],
            rotation: (rotation.degrees() as f32).to_radians(),
            opaque: sprite.opaque,
            ..Quad::default()
        };
        self.quads.paint(
            &self.queue,
            encoder,
            target,
            self.format,
            target_size,
            &sprite.bound,
            quad,
        );
    }
}
