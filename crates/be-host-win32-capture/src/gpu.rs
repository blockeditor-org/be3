// @yeskeepfile: the comments below are the invariants the FFI relies on, which
// nothing here can test (no Windows machine runs these crates' tests).

use std::ffi::CStr;
use std::sync::{Arc, Mutex, PoisonError};

use ash::vk;
use wgpu::hal::api::Vulkan as Api;
use windows::Win32::Foundation::{GENERIC_ALL, HMODULE};
use windows::Win32::Graphics::Direct3D::{
    D3D_DRIVER_TYPE_UNKNOWN, D3D_FEATURE_LEVEL_11_0, D3D_FEATURE_LEVEL_11_1,
};
use windows::Win32::Graphics::Direct3D11::{
    D3D11_BIND_RENDER_TARGET, D3D11_BIND_SHADER_RESOURCE, D3D11_CREATE_DEVICE_BGRA_SUPPORT,
    D3D11_FENCE_FLAG_SHARED, D3D11_RESOURCE_MISC_SHARED, D3D11_RESOURCE_MISC_SHARED_NTHANDLE,
    D3D11_SDK_VERSION, D3D11_TEXTURE2D_DESC, D3D11_USAGE_DEFAULT, D3D11CreateDevice, ID3D11Device,
    ID3D11Device5, ID3D11DeviceContext, ID3D11DeviceContext4, ID3D11Fence, ID3D11Multithread,
    ID3D11Texture2D,
};
use windows::Win32::Graphics::Dxgi::Common::{
    DXGI_FORMAT, DXGI_FORMAT_B8G8R8A8_UNORM, DXGI_FORMAT_B8G8R8A8_UNORM_SRGB, DXGI_SAMPLE_DESC,
};
use windows::Win32::Graphics::Dxgi::{
    CreateDXGIFactory1, DXGI_ERROR_NOT_FOUND, DXGI_SHARED_RESOURCE_READ,
    DXGI_SHARED_RESOURCE_WRITE, IDXGIAdapter1, IDXGIFactory1, IDXGIResource1,
};
use windows::core::{Interface, PCWSTR};

use crate::handle::NtHandle;
use crate::{Luid, Size, adapter_matching, luid, memory_type};

// wgpu-hal already enables VK_KHR_external_memory_win32 wherever the adapter
// has it; VK_KHR_external_semaphore_win32 it never enables, so the device must
// be opened with these (beui's `OpenDevice::extensions`).
pub const DEVICE_EXTENSIONS: [&CStr; 2] = [
    ash::khr::external_memory_win32::NAME,
    ash::khr::external_semaphore_win32::NAME,
];

const MEMORY: vk::ExternalMemoryHandleTypeFlags = vk::ExternalMemoryHandleTypeFlags::D3D11_TEXTURE;
const FENCE: vk::ExternalSemaphoreHandleTypeFlags =
    vk::ExternalSemaphoreHandleTypeFlags::D3D12_FENCE;
const IMAGE_USAGE: vk::ImageUsageFlags = vk::ImageUsageFlags::from_raw(
    vk::ImageUsageFlags::SAMPLED.as_raw() | vk::ImageUsageFlags::TRANSFER_SRC.as_raw(),
);

#[derive(Clone)]
pub struct Device(pub(crate) Arc<Inner>);

pub(crate) struct Inner {
    // One device for every capture. Its immediate context is shared by the
    // capture threads, which is sound only because `SetMultithreadProtected`
    // makes each of its calls take the context's lock.
    pub(crate) d3d: ID3D11Device5,
    pub(crate) context: ID3D11DeviceContext4,
    device: wgpu::Device,
    queue: wgpu::Queue,
    srgb: bool,
    adapter: String,
    vulkan: Vulkan,
    retired: Mutex<Vec<Retired>>,
}

struct Vulkan {
    instance: ash::Instance,
    physical: vk::PhysicalDevice,
    device: ash::Device,
    memory: ash::khr::external_memory_win32::Device,
    semaphore: ash::khr::external_semaphore_win32::Device,
}

// A slot's release fence: a Direct3D 11 fence shared by NT handle and imported
// as a Vulkan timeline semaphore. Only Vulkan signals it, after the last
// submission that sampled the slot; Direct3D reads its completed value before
// it writes the slot again.
pub(crate) struct Release {
    pub(crate) fence: ID3D11Fence,
    pub(crate) semaphore: vk::Semaphore,
}

pub(crate) struct Texture {
    pub(crate) d3d: ID3D11Texture2D,
    pub(crate) wgpu: wgpu::Texture,
    pub(crate) size: Size,
}

struct Retired {
    release: Release,
    value: u64,
}

fn formats(srgb: bool) -> (DXGI_FORMAT, vk::Format, wgpu::TextureFormat) {
    match srgb {
        false => (
            DXGI_FORMAT_B8G8R8A8_UNORM,
            vk::Format::B8G8R8A8_UNORM,
            wgpu::TextureFormat::Bgra8Unorm,
        ),
        true => (
            DXGI_FORMAT_B8G8R8A8_UNORM_SRGB,
            vk::Format::B8G8R8A8_SRGB,
            wgpu::TextureFormat::Bgra8UnormSrgb,
        ),
    }
}

fn failed(what: &'static str) -> impl FnOnce(windows::core::Error) -> String {
    move |error| format!("{what}: {error}")
}

impl Device {
    pub fn new(device: &wgpu::Device, queue: &wgpu::Queue, srgb: bool) -> Result<Device, String> {
        let (vulkan, wanted) = vulkan_of(device, formats(srgb).1)?;
        let (d3d, context, adapter) = d3d11_on(wanted)?;
        Ok(Device(Arc::new(Inner {
            d3d,
            context,
            device: device.clone(),
            queue: queue.clone(),
            srgb,
            adapter,
            vulkan,
            retired: Mutex::new(Vec::new()),
        })))
    }

    pub fn texture_format(&self) -> wgpu::TextureFormat {
        formats(self.0.srgb).2
    }

    pub fn adapter(&self) -> &str {
        &self.0.adapter
    }
}

// unsafe(ffi)
fn vulkan_of(device: &wgpu::Device, format: vk::Format) -> Result<(Vulkan, Luid), String> {
    let hal = unsafe { device.as_hal::<Api>() }.ok_or("the wgpu device is not a Vulkan device")?;
    let enabled = hal.enabled_device_extensions();
    if let Some(missing) = DEVICE_EXTENSIONS
        .iter()
        .find(|name| !enabled.contains(name))
    {
        return Err(format!(
            "the Vulkan device was opened without {}",
            missing.to_string_lossy()
        ));
    }
    let shared = hal.shared_instance();
    let instance = shared.raw_instance().clone();
    let physical = hal.raw_physical_device();
    let properties = unsafe { instance.get_physical_device_properties(physical) };
    let api = properties.api_version.min(shared.instance_api_version());
    // vkGetPhysicalDeviceProperties2 and the external capability queries are
    // core in 1.1; ash panics on a null function pointer, so check first.
    if api < vk::API_VERSION_1_1 {
        return Err("sharing textures with Direct3D needs Vulkan 1.1".to_owned());
    }
    let mut id = vk::PhysicalDeviceIDProperties::default();
    let mut properties2 = vk::PhysicalDeviceProperties2::default().push_next(&mut id);
    unsafe { instance.get_physical_device_properties2(physical, &mut properties2) };
    if id.device_luid_valid == vk::FALSE {
        return Err("the Vulkan device has no LUID to find its Direct3D adapter by".to_owned());
    }
    // wgpu-hal turns the timelineSemaphore feature on whenever the device
    // reports it and has 1.2 or VK_KHR_timeline_semaphore, so this says whether
    // the semaphores imported below may be timeline semaphores.
    let mut timeline = vk::PhysicalDeviceTimelineSemaphoreFeatures::default();
    let mut features = vk::PhysicalDeviceFeatures2::default().push_next(&mut timeline);
    unsafe { instance.get_physical_device_features2(physical, &mut features) };
    let timelines = (api >= vk::API_VERSION_1_2
        || enabled.contains(&ash::khr::timeline_semaphore::NAME))
        && timeline.timeline_semaphore == vk::TRUE;
    if !timelines {
        return Err("the Vulkan device has no timeline semaphores".to_owned());
    }
    let mut kind =
        vk::SemaphoreTypeCreateInfo::default().semaphore_type(vk::SemaphoreType::TIMELINE);
    let info = vk::PhysicalDeviceExternalSemaphoreInfo::default()
        .handle_type(FENCE)
        .push_next(&mut kind);
    let mut external = vk::ExternalSemaphoreProperties::default();
    unsafe {
        instance.get_physical_device_external_semaphore_properties(physical, &info, &mut external)
    };
    if !external
        .external_semaphore_features
        .contains(vk::ExternalSemaphoreFeatureFlags::IMPORTABLE)
    {
        return Err("the Vulkan device cannot import Direct3D fences".to_owned());
    }
    let mut external = vk::PhysicalDeviceExternalImageFormatInfo::default().handle_type(MEMORY);
    let info = vk::PhysicalDeviceImageFormatInfo2::default()
        .format(format)
        .ty(vk::ImageType::TYPE_2D)
        .tiling(vk::ImageTiling::OPTIMAL)
        .usage(IMAGE_USAGE)
        .push_next(&mut external);
    let mut external_properties = vk::ExternalImageFormatProperties::default();
    let mut image_properties =
        vk::ImageFormatProperties2::default().push_next(&mut external_properties);
    let supported = unsafe {
        instance.get_physical_device_image_format_properties2(
            physical,
            &info,
            &mut image_properties,
        )
    };
    if supported.is_err()
        || !external_properties
            .external_memory_properties
            .external_memory_features
            .contains(vk::ExternalMemoryFeatureFlags::IMPORTABLE)
    {
        return Err("the Vulkan device cannot import Direct3D 11 textures".to_owned());
    }
    let raw = hal.raw_device().clone();
    let vulkan = Vulkan {
        memory: ash::khr::external_memory_win32::Device::new(&instance, &raw),
        semaphore: ash::khr::external_semaphore_win32::Device::new(&instance, &raw),
        instance,
        physical,
        device: raw,
    };
    Ok((vulkan, id.device_luid))
}

// unsafe(ffi)
fn d3d11_on(wanted: Luid) -> Result<(ID3D11Device5, ID3D11DeviceContext4, String), String> {
    let factory: IDXGIFactory1 =
        unsafe { CreateDXGIFactory1() }.map_err(failed("DXGI did not start"))?;
    let mut adapters: Vec<(Luid, IDXGIAdapter1, String)> = Vec::new();
    for index in 0u32.. {
        match unsafe { factory.EnumAdapters1(index) } {
            Ok(adapter) => {
                if let Ok(desc) = unsafe { adapter.GetDesc1() } {
                    let id = luid(desc.AdapterLuid.LowPart, desc.AdapterLuid.HighPart);
                    let end = desc
                        .Description
                        .iter()
                        .position(|&unit| unit == 0)
                        .unwrap_or(desc.Description.len());
                    let name = format!(
                        "{} (LUID {:08x}:{:08x})",
                        String::from_utf16_lossy(&desc.Description[..end]),
                        desc.AdapterLuid.HighPart,
                        desc.AdapterLuid.LowPart
                    );
                    adapters.push((id, adapter, name));
                }
            }
            Err(error) if error.code() == DXGI_ERROR_NOT_FOUND => break,
            Err(error) => return Err(failed("DXGI could not list its adapters")(error)),
        }
    }
    let index = adapter_matching(wanted, adapters.iter().map(|(id, _, _)| *id))
        .ok_or("no Direct3D adapter is the GPU wgpu draws with")?;
    let mut device: Option<ID3D11Device> = None;
    let mut context: Option<ID3D11DeviceContext> = None;
    // D3D_DRIVER_TYPE_UNKNOWN is required when an adapter is given.
    unsafe {
        D3D11CreateDevice(
            &adapters[index].1,
            D3D_DRIVER_TYPE_UNKNOWN,
            HMODULE::default(),
            D3D11_CREATE_DEVICE_BGRA_SUPPORT,
            Some(&[D3D_FEATURE_LEVEL_11_1, D3D_FEATURE_LEVEL_11_0]),
            D3D11_SDK_VERSION,
            Some(&mut device),
            None,
            Some(&mut context),
        )
    }
    .map_err(failed("the Direct3D 11 device could not be created"))?;
    let device = device.ok_or("Direct3D 11 made no device")?;
    let context = context.ok_or("Direct3D 11 made no device context")?;
    let multithread: ID3D11Multithread = context.cast().map_err(failed(
        "the Direct3D 11 context cannot be shared between threads",
    ))?;
    let _ = unsafe { multithread.SetMultithreadProtected(true) };
    // ID3D11Device5 (fences) and ID3D11DeviceContext4 (Signal) are Windows 10
    // 1703 and later, as Windows.Graphics.Capture's frame pools are anyway.
    let device: ID3D11Device5 = device
        .cast()
        .map_err(failed("Direct3D 11 has no fences on this system"))?;
    let context: ID3D11DeviceContext4 = context
        .cast()
        .map_err(failed("Direct3D 11 has no fences on this system"))?;
    Ok((device, context, adapters.swap_remove(index).2))
}

// unsafe(ffi)
impl Inner {
    pub(crate) fn release(&self) -> Result<Release, String> {
        let mut fence: Option<ID3D11Fence> = None;
        unsafe { self.d3d.CreateFence(0, D3D11_FENCE_FLAG_SHARED, &mut fence) }
            .map_err(failed("a shared fence could not be created"))?;
        let fence = fence.ok_or("Direct3D 11 made no fence")?;
        let handle = NtHandle(
            unsafe { fence.CreateSharedHandle(None, GENERIC_ALL.0, PCWSTR::null()) }
                .map_err(failed("the fence could not be shared"))?,
        );
        let mut kind = vk::SemaphoreTypeCreateInfo::default()
            .semaphore_type(vk::SemaphoreType::TIMELINE)
            .initial_value(0);
        let info = vk::SemaphoreCreateInfo::default().push_next(&mut kind);
        let semaphore = unsafe { self.vulkan.device.create_semaphore(&info, None) }
            .map_err(|error| format!("a semaphore could not be created: {error}"))?;
        // A permanent import: the semaphore now shares the fence's payload.
        // Importing an NT handle does not take ownership of it, so `handle`
        // still closes it when it drops at the end of this function.
        let import = vk::ImportSemaphoreWin32HandleInfoKHR::default()
            .semaphore(semaphore)
            .handle_type(FENCE)
            .handle(handle.0.0 as vk::HANDLE);
        if let Err(error) = unsafe { self.vulkan.semaphore.import_semaphore_win32_handle(&import) }
        {
            unsafe { self.vulkan.device.destroy_semaphore(semaphore, None) };
            return Err(format!("the shared fence could not be imported: {error}"));
        }
        Ok(Release { fence, semaphore })
    }

    pub(crate) fn texture(&self, size: Size) -> Result<Texture, String> {
        let (dxgi_format, vk_format, wgpu_format) = formats(self.srgb);
        // SHARED_NTHANDLE needs SHARED (or a keyed mutex) beside it; the
        // fences order the two APIs, so no keyed mutex.
        let desc = D3D11_TEXTURE2D_DESC {
            Width: size.width,
            Height: size.height,
            MipLevels: 1,
            ArraySize: 1,
            Format: dxgi_format,
            SampleDesc: DXGI_SAMPLE_DESC {
                Count: 1,
                Quality: 0,
            },
            Usage: D3D11_USAGE_DEFAULT,
            BindFlags: (D3D11_BIND_SHADER_RESOURCE.0 | D3D11_BIND_RENDER_TARGET.0) as u32,
            CPUAccessFlags: 0,
            MiscFlags: (D3D11_RESOURCE_MISC_SHARED.0 | D3D11_RESOURCE_MISC_SHARED_NTHANDLE.0)
                as u32,
        };
        let mut texture: Option<ID3D11Texture2D> = None;
        unsafe { self.d3d.CreateTexture2D(&desc, None, Some(&mut texture)) }
            .map_err(failed("a shared texture could not be created"))?;
        let texture = texture.ok_or("Direct3D 11 made no texture")?;
        let resource: IDXGIResource1 = texture
            .cast()
            .map_err(failed("the texture is not a DXGI resource"))?;
        let handle = NtHandle(
            unsafe {
                resource.CreateSharedHandle(
                    None,
                    DXGI_SHARED_RESOURCE_READ.0 | DXGI_SHARED_RESOURCE_WRITE.0,
                    PCWSTR::null(),
                )
            }
            .map_err(failed("the texture could not be shared"))?,
        );
        let wgpu = self.import(handle, size, vk_format, wgpu_format)?;
        Ok(Texture {
            d3d: texture,
            wgpu,
            size,
        })
    }

    fn import(
        &self,
        handle: NtHandle,
        size: Size,
        vk_format: vk::Format,
        wgpu_format: wgpu::TextureFormat,
    ) -> Result<wgpu::Texture, String> {
        let device = &self.vulkan.device;
        // The image must describe the Direct3D texture exactly: same format,
        // size, one mip level, one layer, one sample.
        let mut external = vk::ExternalMemoryImageCreateInfo::default().handle_types(MEMORY);
        let info = vk::ImageCreateInfo::default()
            .image_type(vk::ImageType::TYPE_2D)
            .format(vk_format)
            .extent(vk::Extent3D {
                width: size.width,
                height: size.height,
                depth: 1,
            })
            .mip_levels(1)
            .array_layers(1)
            .samples(vk::SampleCountFlags::TYPE_1)
            .tiling(vk::ImageTiling::OPTIMAL)
            .usage(IMAGE_USAGE)
            .sharing_mode(vk::SharingMode::EXCLUSIVE)
            .initial_layout(vk::ImageLayout::UNDEFINED)
            .push_next(&mut external);
        let image = unsafe { device.create_image(&info, None) }
            .map_err(|error| format!("the image could not be created: {error}"))?;
        let memory = match self.bind(image, &handle) {
            Ok(memory) => memory,
            Err(error) => {
                unsafe { device.destroy_image(image, None) };
                return Err(error);
            }
        };
        let extent = wgpu::Extent3d {
            width: size.width,
            height: size.height,
            depth_or_array_layers: 1,
        };
        let hal_descriptor = wgpu::hal::TextureDescriptor {
            label: Some("window capture"),
            size: extent,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu_format,
            usage: wgpu::TextureUses::RESOURCE | wgpu::TextureUses::COPY_SRC,
            memory_flags: wgpu::hal::MemoryFlags::empty(),
            view_formats: Vec::new(),
        };
        let raw = device.clone();
        // wgpu runs the callback once the texture is dropped and no submission
        // uses it any more. The NT handle stays open until then, so the shared
        // resource outlives every Vulkan use of it whatever the driver does.
        let texture = unsafe {
            let hal = self
                .device
                .as_hal::<Api>()
                .ok_or("the device is not a Vulkan device")?;
            let hal_texture = hal.texture_from_raw(
                image,
                &hal_descriptor,
                Some(Box::new(move || {
                    raw.destroy_image(image, None);
                    raw.free_memory(memory, None);
                    drop(handle);
                })),
                wgpu::hal::vulkan::TextureMemory::External,
            );
            drop(hal);
            self.device.create_texture_from_hal::<Api>(
                hal_texture,
                &wgpu::TextureDescriptor {
                    label: Some("window capture"),
                    size: extent,
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: wgpu::TextureDimension::D2,
                    format: wgpu_format,
                    usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_SRC,
                    view_formats: &[],
                },
            )
        };
        Ok(texture)
    }

    fn bind(&self, image: vk::Image, handle: &NtHandle) -> Result<vk::DeviceMemory, String> {
        let device = &self.vulkan.device;
        let raw = handle.0.0 as vk::HANDLE;
        let requirements = unsafe { device.get_image_memory_requirements(image) };
        let mut handle_properties = vk::MemoryWin32HandlePropertiesKHR::default();
        let importable = match unsafe {
            self.vulkan.memory.get_memory_win32_handle_properties(
                MEMORY,
                raw,
                &mut handle_properties,
            )
        } {
            Ok(()) => handle_properties.memory_type_bits,
            Err(_) => u32::MAX,
        };
        let memory_properties = unsafe {
            self.vulkan
                .instance
                .get_physical_device_memory_properties(self.vulkan.physical)
        };
        let index = memory_type(requirements.memory_type_bits & importable, |index| {
            memory_properties.memory_types[index as usize]
                .property_flags
                .contains(vk::MemoryPropertyFlags::DEVICE_LOCAL)
        })
        .ok_or("no memory type can hold the shared texture")?;
        // Direct3D 11 textures must be imported as dedicated allocations; the
        // allocation size is ignored for this handle type.
        let mut import = vk::ImportMemoryWin32HandleInfoKHR::default()
            .handle_type(MEMORY)
            .handle(raw);
        let mut dedicated = vk::MemoryDedicatedAllocateInfo::default().image(image);
        let allocation = vk::MemoryAllocateInfo::default()
            .allocation_size(requirements.size)
            .memory_type_index(index)
            .push_next(&mut import)
            .push_next(&mut dedicated);
        let memory = unsafe { device.allocate_memory(&allocation, None) }
            .map_err(|error| format!("the shared texture could not be imported: {error}"))?;
        if let Err(error) = unsafe { device.bind_image_memory(image, memory, 0) } {
            unsafe { device.free_memory(memory, None) };
            return Err(format!("the shared texture could not be bound: {error}"));
        }
        Ok(memory)
    }

    // The signal joins the queue's next submission, which comes after every
    // submission that sampled the slot, and a semaphore signal waits for all
    // work submitted before it. A slot never has two signals outstanding
    // (`Ring::release`), so the values reach the semaphore in order.
    pub(crate) fn signal(&self, semaphore: vk::Semaphore, value: u64) {
        if let Some(queue) = unsafe { self.queue.as_hal::<Api>() } {
            queue.add_signal_semaphore(semaphore, Some(value));
        }
    }

    // A semaphore can be destroyed only once no submission refers to it, so
    // one whose last signal has not happened waits here until it has.
    pub(crate) fn retire(&self, release: Release, value: u64) {
        let mut retired = self.retired.lock().unwrap_or_else(PoisonError::into_inner);
        retired.push(Retired { release, value });
        self.sweep(&mut retired);
    }

    fn sweep(&self, retired: &mut Vec<Retired>) {
        retired.retain(|entry| {
            let done = unsafe { entry.release.fence.GetCompletedValue() } >= entry.value;
            if done {
                unsafe {
                    self.vulkan
                        .device
                        .destroy_semaphore(entry.release.semaphore, None)
                };
            }
            !done
        });
    }
}

impl Drop for Inner {
    fn drop(&mut self) {
        let mut retired = std::mem::take(
            self.retired
                .get_mut()
                .unwrap_or_else(PoisonError::into_inner),
        );
        self.sweep(&mut retired);
        // What is left still has a signal queued that never ran: destroying
        // its semaphore could break that submission, so it is leaked.
        std::mem::forget(retired);
    }
}
