use std::ffi::CStr;
use std::os::fd::{AsFd, AsRawFd, IntoRawFd};

use ash::vk;
use smithay::backend::allocator::dmabuf::Dmabuf;
use smithay::backend::allocator::{Buffer as _, Format, Fourcc, Modifier};
use wgpu::hal::api::Vulkan as Api;

const WANTED_EXTENSIONS: [&CStr; 4] = [
    ash::ext::image_drm_format_modifier::NAME,
    ash::khr::external_semaphore_fd::NAME,
    ash::ext::queue_family_foreign::NAME,
    ash::ext::physical_device_drm::NAME,
];

const FORMATS: [Fourcc; 4] = [
    Fourcc::Argb8888,
    Fourcc::Xrgb8888,
    Fourcc::Abgr8888,
    Fourcc::Xbgr8888,
];

pub fn open_device(
    adapter: &wgpu::Adapter,
    descriptor: &wgpu::DeviceDescriptor<'_>,
) -> Option<(wgpu::Device, wgpu::Queue)> {
    let hal = unsafe { adapter.as_hal::<Api>() }?;
    let capabilities = hal.physical_device_capabilities();
    let extra: Vec<&'static CStr> = WANTED_EXTENSIONS
        .into_iter()
        .filter(|name| capabilities.supports_extension(name))
        .collect();
    let opened = unsafe {
        hal.open_with_callback(
            descriptor.required_features,
            &descriptor.required_limits,
            &descriptor.memory_hints,
            Some(Box::new(move |arguments| {
                for name in extra {
                    if !arguments.extensions.contains(&name) {
                        arguments.extensions.push(name);
                    }
                }
            })),
        )
    }
    .ok()?;
    drop(hal);
    unsafe { adapter.create_device_from_hal(opened, descriptor) }.ok()
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Usage {
    Sample,
    Render,
}

impl Usage {
    fn vulkan(self) -> vk::ImageUsageFlags {
        match self {
            Usage::Sample => vk::ImageUsageFlags::SAMPLED | vk::ImageUsageFlags::TRANSFER_SRC,
            Usage::Render => {
                vk::ImageUsageFlags::COLOR_ATTACHMENT
                    | vk::ImageUsageFlags::TRANSFER_DST
                    | vk::ImageUsageFlags::TRANSFER_SRC
            }
        }
    }

    fn hal(self) -> wgpu::TextureUses {
        match self {
            Usage::Sample => wgpu::TextureUses::RESOURCE | wgpu::TextureUses::COPY_SRC,
            Usage::Render => {
                wgpu::TextureUses::COLOR_TARGET
                    | wgpu::TextureUses::COPY_DST
                    | wgpu::TextureUses::COPY_SRC
            }
        }
    }

    fn wgpu(self) -> wgpu::TextureUsages {
        match self {
            Usage::Sample => wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_SRC,
            Usage::Render => {
                wgpu::TextureUsages::RENDER_ATTACHMENT
                    | wgpu::TextureUsages::COPY_DST
                    | wgpu::TextureUsages::COPY_SRC
            }
        }
    }
}

pub struct Imported {
    pub texture: wgpu::Texture,
    pub opaque: bool,
}

#[derive(Clone)]
pub struct Vulkan {
    instance: ash::Instance,
    device: ash::Device,
    physical: vk::PhysicalDevice,
    memory_fd: ash::khr::external_memory_fd::Device,
    modifiers: bool,
    render_node: Option<u64>,
}

pub fn texture_format(code: Fourcc, srgb: bool) -> Option<(vk::Format, wgpu::TextureFormat, bool)> {
    let (vulkan, wgpu, opaque) = match (code, srgb) {
        (Fourcc::Argb8888, false) => (
            vk::Format::B8G8R8A8_UNORM,
            wgpu::TextureFormat::Bgra8Unorm,
            false,
        ),
        (Fourcc::Argb8888, true) => (
            vk::Format::B8G8R8A8_SRGB,
            wgpu::TextureFormat::Bgra8UnormSrgb,
            false,
        ),
        (Fourcc::Xrgb8888, false) => (
            vk::Format::B8G8R8A8_UNORM,
            wgpu::TextureFormat::Bgra8Unorm,
            true,
        ),
        (Fourcc::Xrgb8888, true) => (
            vk::Format::B8G8R8A8_SRGB,
            wgpu::TextureFormat::Bgra8UnormSrgb,
            true,
        ),
        (Fourcc::Abgr8888, false) => (
            vk::Format::R8G8B8A8_UNORM,
            wgpu::TextureFormat::Rgba8Unorm,
            false,
        ),
        (Fourcc::Abgr8888, true) => (
            vk::Format::R8G8B8A8_SRGB,
            wgpu::TextureFormat::Rgba8UnormSrgb,
            false,
        ),
        (Fourcc::Xbgr8888, false) => (
            vk::Format::R8G8B8A8_UNORM,
            wgpu::TextureFormat::Rgba8Unorm,
            true,
        ),
        (Fourcc::Xbgr8888, true) => (
            vk::Format::R8G8B8A8_SRGB,
            wgpu::TextureFormat::Rgba8UnormSrgb,
            true,
        ),
        _ => return None,
    };
    Some((vulkan, wgpu, opaque))
}

impl Vulkan {
    pub fn of(device: &wgpu::Device) -> Option<Self> {
        let hal = unsafe { device.as_hal::<Api>() }?;
        let enabled = hal.enabled_device_extensions();
        let external = [
            ash::khr::external_memory_fd::NAME,
            ash::ext::external_memory_dma_buf::NAME,
        ];
        if !external.iter().all(|name| enabled.contains(name)) {
            return None;
        }
        let modifiers = enabled.contains(&ash::ext::image_drm_format_modifier::NAME);
        let instance = hal.shared_instance().raw_instance().clone();
        let raw = hal.raw_device().clone();
        let physical = hal.raw_physical_device();
        let memory_fd = ash::khr::external_memory_fd::Device::new(&instance, &raw);
        let render_node = render_node(&instance, physical);
        Some(Self {
            instance,
            device: raw,
            physical,
            memory_fd,
            modifiers,
            render_node,
        })
    }

    pub fn render_node(&self) -> Option<u64> {
        self.render_node
    }

    pub fn formats(&self, usage: Usage) -> Vec<Format> {
        let mut formats = Vec::new();
        for code in FORMATS {
            let Some((format, _, _)) = texture_format(code, false) else {
                continue;
            };
            for modifier in self.modifiers_of(format, usage) {
                formats.push(Format { code, modifier });
            }
        }
        formats
    }

    fn modifiers_of(&self, format: vk::Format, usage: Usage) -> Vec<Modifier> {
        if !self.modifiers {
            return match self.importable(format, usage, None) {
                true => vec![Modifier::Linear],
                false => Vec::new(),
            };
        }
        let mut list = vk::DrmFormatModifierPropertiesListEXT::default();
        let mut properties = vk::FormatProperties2::default().push_next(&mut list);
        unsafe {
            self.instance.get_physical_device_format_properties2(
                self.physical,
                format,
                &mut properties,
            )
        };
        let count = list.drm_format_modifier_count as usize;
        let mut entries = vec![vk::DrmFormatModifierPropertiesEXT::default(); count];
        let mut list = vk::DrmFormatModifierPropertiesListEXT::default()
            .drm_format_modifier_properties(&mut entries);
        let mut properties = vk::FormatProperties2::default().push_next(&mut list);
        unsafe {
            self.instance.get_physical_device_format_properties2(
                self.physical,
                format,
                &mut properties,
            )
        };
        let needed = match usage {
            Usage::Sample => vk::FormatFeatureFlags::SAMPLED_IMAGE,
            Usage::Render => vk::FormatFeatureFlags::COLOR_ATTACHMENT,
        };
        entries
            .into_iter()
            .filter(|entry| {
                entry.drm_format_modifier_plane_count == 1
                    && entry.drm_format_modifier_tiling_features.contains(needed)
                    && self.importable(format, usage, Some(entry.drm_format_modifier))
            })
            .map(|entry| Modifier::from(entry.drm_format_modifier))
            .collect()
    }

    fn importable(&self, format: vk::Format, usage: Usage, modifier: Option<u64>) -> bool {
        let mut external = vk::PhysicalDeviceExternalImageFormatInfo::default()
            .handle_type(vk::ExternalMemoryHandleTypeFlags::DMA_BUF_EXT);
        let mut modifier_info = vk::PhysicalDeviceImageDrmFormatModifierInfoEXT::default()
            .drm_format_modifier(modifier.unwrap_or_default())
            .sharing_mode(vk::SharingMode::EXCLUSIVE);
        let mut info = vk::PhysicalDeviceImageFormatInfo2::default()
            .format(format)
            .ty(vk::ImageType::TYPE_2D)
            .usage(usage.vulkan())
            .push_next(&mut external);
        info = match modifier {
            Some(_) => info
                .tiling(vk::ImageTiling::DRM_FORMAT_MODIFIER_EXT)
                .push_next(&mut modifier_info),
            None => info.tiling(vk::ImageTiling::LINEAR),
        };
        let mut external_properties = vk::ExternalImageFormatProperties::default();
        let mut properties =
            vk::ImageFormatProperties2::default().push_next(&mut external_properties);
        let supported = unsafe {
            self.instance.get_physical_device_image_format_properties2(
                self.physical,
                &info,
                &mut properties,
            )
        };
        supported.is_ok()
            && external_properties
                .external_memory_properties
                .external_memory_features
                .contains(vk::ExternalMemoryFeatureFlags::IMPORTABLE)
    }

    pub fn import(
        &self,
        device: &wgpu::Device,
        dmabuf: &Dmabuf,
        srgb: bool,
        usage: Usage,
    ) -> Result<Imported, String> {
        let format = dmabuf.format();
        let (vk_format, wgpu_format, opaque) = texture_format(format.code, srgb)
            .ok_or_else(|| format!("{:?} is not a format the compositor reads", format.code))?;
        if dmabuf.num_planes() != 1 {
            return Err("only single-plane buffers are supported".to_owned());
        }
        let size = dmabuf.size();
        let (width, height) = (size.w.max(1) as u32, size.h.max(1) as u32);
        let fd = dmabuf.handles().next().ok_or("the buffer has no planes")?;
        let offset = u64::from(dmabuf.offsets().next().unwrap_or(0));
        let stride = u64::from(dmabuf.strides().next().unwrap_or(0));
        let modifier: u64 = format.modifier.into();
        let explicit = self.modifiers && format.modifier != Modifier::Invalid;
        if !explicit && format.modifier != Modifier::Linear {
            return Err(format!(
                "{:?} buffers need VK_EXT_image_drm_format_modifier",
                format.modifier
            ));
        }
        let layouts = [vk::SubresourceLayout {
            offset,
            size: 0,
            row_pitch: stride,
            array_pitch: 0,
            depth_pitch: 0,
        }];
        let mut modifier_info = vk::ImageDrmFormatModifierExplicitCreateInfoEXT::default()
            .drm_format_modifier(modifier)
            .plane_layouts(&layouts);
        let mut external = vk::ExternalMemoryImageCreateInfo::default()
            .handle_types(vk::ExternalMemoryHandleTypeFlags::DMA_BUF_EXT);
        let mut info = vk::ImageCreateInfo::default()
            .image_type(vk::ImageType::TYPE_2D)
            .format(vk_format)
            .extent(vk::Extent3D {
                width,
                height,
                depth: 1,
            })
            .mip_levels(1)
            .array_layers(1)
            .samples(vk::SampleCountFlags::TYPE_1)
            .usage(usage.vulkan())
            .sharing_mode(vk::SharingMode::EXCLUSIVE)
            .initial_layout(vk::ImageLayout::UNDEFINED)
            .push_next(&mut external);
        info = match explicit {
            true => info
                .tiling(vk::ImageTiling::DRM_FORMAT_MODIFIER_EXT)
                .push_next(&mut modifier_info),
            false => info.tiling(vk::ImageTiling::LINEAR),
        };
        let image = unsafe { self.device.create_image(&info, None) }
            .map_err(|error| format!("the image could not be created: {error}"))?;
        let bound = self.bind(image, fd, offset, stride, explicit);
        let memory = match bound {
            Ok(memory) => memory,
            Err(error) => {
                unsafe { self.device.destroy_image(image, None) };
                return Err(error);
            }
        };
        let raw = self.device.clone();
        let extent = wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        };
        let hal_descriptor = wgpu::hal::TextureDescriptor {
            label: Some("client dmabuf"),
            size: extent,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu_format,
            usage: usage.hal(),
            memory_flags: wgpu::hal::MemoryFlags::empty(),
            view_formats: Vec::new(),
        };
        let texture = unsafe {
            let hal = device
                .as_hal::<Api>()
                .ok_or("the device is not a Vulkan device")?;
            let hal_texture = hal.texture_from_raw(
                image,
                &hal_descriptor,
                Some(Box::new(move || {
                    raw.destroy_image(image, None);
                    raw.free_memory(memory, None);
                })),
                wgpu::hal::vulkan::TextureMemory::External,
            );
            drop(hal);
            device.create_texture_from_hal::<Api>(
                hal_texture,
                &wgpu::TextureDescriptor {
                    label: Some("client dmabuf"),
                    size: extent,
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: wgpu::TextureDimension::D2,
                    format: wgpu_format,
                    usage: usage.wgpu(),
                    view_formats: &[],
                },
            )
        };
        Ok(Imported { texture, opaque })
    }

    fn bind(
        &self,
        image: vk::Image,
        fd: std::os::fd::BorrowedFd<'_>,
        offset: u64,
        stride: u64,
        explicit: bool,
    ) -> Result<vk::DeviceMemory, String> {
        if !explicit {
            let layout = unsafe {
                self.device.get_image_subresource_layout(
                    image,
                    vk::ImageSubresource {
                        aspect_mask: vk::ImageAspectFlags::COLOR,
                        mip_level: 0,
                        array_layer: 0,
                    },
                )
            };
            if layout.row_pitch != stride || offset != 0 {
                return Err(format!(
                    "a linear buffer with stride {stride} at offset {offset} does not match the \
                     device's layout of stride {} without VK_EXT_image_drm_format_modifier",
                    layout.row_pitch
                ));
            }
        }
        let owned = fd
            .try_clone_to_owned()
            .map_err(|error| format!("the buffer could not be duplicated: {error}"))?;
        let mut fd_properties = vk::MemoryFdPropertiesKHR::default();
        unsafe {
            self.memory_fd.get_memory_fd_properties(
                vk::ExternalMemoryHandleTypeFlags::DMA_BUF_EXT,
                owned.as_fd().as_raw_fd(),
                &mut fd_properties,
            )
        }
        .map_err(|error| format!("the buffer's memory could not be queried: {error}"))?;
        let requirements = unsafe { self.device.get_image_memory_requirements(image) };
        let types = requirements.memory_type_bits & fd_properties.memory_type_bits;
        if types == 0 {
            return Err("no memory type can hold the buffer".to_owned());
        }
        let mut import = vk::ImportMemoryFdInfoKHR::default()
            .handle_type(vk::ExternalMemoryHandleTypeFlags::DMA_BUF_EXT)
            .fd(owned.into_raw_fd());
        let mut dedicated = vk::MemoryDedicatedAllocateInfo::default().image(image);
        let allocation = vk::MemoryAllocateInfo::default()
            .allocation_size(requirements.size)
            .memory_type_index(types.trailing_zeros())
            .push_next(&mut import)
            .push_next(&mut dedicated);
        let memory =
            unsafe { self.device.allocate_memory(&allocation, None) }.map_err(|error| {
                unsafe { libc_close(import.fd) };
                format!("the buffer's memory could not be imported: {error}")
            })?;
        if let Err(error) = unsafe { self.device.bind_image_memory(image, memory, 0) } {
            unsafe { self.device.free_memory(memory, None) };
            return Err(format!("the buffer's memory could not be bound: {error}"));
        }
        Ok(memory)
    }
}

unsafe fn libc_close(fd: std::os::fd::RawFd) {
    drop(unsafe { <std::os::fd::OwnedFd as std::os::fd::FromRawFd>::from_raw_fd(fd) });
}

fn render_node(instance: &ash::Instance, physical: vk::PhysicalDevice) -> Option<u64> {
    let supported = unsafe { instance.enumerate_device_extension_properties(physical) }.ok()?;
    let has = supported.iter().any(|extension| {
        extension.extension_name_as_c_str() == Ok(ash::ext::physical_device_drm::NAME)
    });
    if !has {
        return None;
    }
    let mut drm = vk::PhysicalDeviceDrmPropertiesEXT::default();
    let mut properties = vk::PhysicalDeviceProperties2::default().push_next(&mut drm);
    unsafe { instance.get_physical_device_properties2(physical, &mut properties) };
    (drm.has_render != 0)
        .then(|| rustix::fs::makedev(drm.render_major as u32, drm.render_minor as u32))
}

#[cfg(test)]
mod tests;
