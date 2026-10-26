use std::error::Error;
use std::ffi::{CStr, c_char};

use ash::vk::{self, Handle};
use beui_renderer_wgpu::present::OpenGpu;
use openxr as xr;
use wgpu::hal::api::Vulkan as Api;

use crate::headset::{Device, Session, Xr};

const LOWEST: u32 = vk::API_VERSION_1_1;
const HIGHEST: u32 = vk::API_VERSION_1_3;

pub(crate) struct Vulkan {
    pub instance: vk::Instance,
    pub physical: vk::PhysicalDevice,
}

fn version(version: xr::Version) -> u32 {
    vk::make_api_version(0, version.major().into(), version.minor().into(), 0)
}

// unsafe(memory)
fn get_instance_proc_addr(entry: &ash::Entry) -> xr::sys::platform::VkGetInstanceProcAddr {
    unsafe { std::mem::transmute(entry.static_fn().get_instance_proc_addr) }
}

// unsafe(ffi)
impl OpenGpu for Xr {
    fn adapter(&self) -> Result<(wgpu::Instance, wgpu::Adapter), Box<dyn Error>> {
        let shared = &self.0;
        let requirements = shared
            .instance
            .graphics_requirements::<xr::Vulkan>(shared.system)
            .map_err(|error| format!("the OpenXR runtime has no Vulkan requirements: {error}"))?;
        let entry = unsafe { ash::Entry::load() }
            .map_err(|error| format!("the Vulkan loader did not load: {error}"))?;
        let driver =
            unsafe { entry.try_enumerate_instance_version() }?.unwrap_or(vk::API_VERSION_1_0);
        let api = driver
            .min(HIGHEST)
            .min(version(requirements.max_api_version_supported) | 0xfff)
            .max(LOWEST);
        if api < version(requirements.min_api_version_supported) {
            return Err(format!(
                "the OpenXR runtime needs Vulkan {}, more than this driver has",
                requirements.min_api_version_supported
            )
            .into());
        }
        let flags = wgpu::InstanceFlags::from_build_config().with_env();
        let extensions = wgpu::hal::vulkan::Instance::desired_extensions(&entry, api, flags)?;
        let names: Vec<*const c_char> = extensions.iter().map(|name| name.as_ptr()).collect();
        let application = vk::ApplicationInfo::default()
            .application_name(&shared.application)
            .engine_name(c"beui")
            .api_version(api);
        let info = vk::InstanceCreateInfo::default()
            .application_info(&application)
            .enabled_extension_names(&names);
        let raw = unsafe {
            shared.instance.create_vulkan_instance(
                shared.system,
                get_instance_proc_addr(&entry),
                (&raw const info).cast(),
            )
        }
        .map_err(|error| format!("OpenXR could not create a Vulkan instance: {error}"))?
        .map_err(|error| {
            format!(
                "OpenXR could not create a Vulkan instance: {:?}",
                vk::Result::from_raw(error)
            )
        })?;
        let handle = vk::Instance::from_raw(raw as u64);
        let ash_instance = unsafe { ash::Instance::load(entry.static_fn(), handle) };
        let physical = unsafe { shared.instance.vulkan_graphics_device(shared.system, raw) }
            .map_err(|error| format!("OpenXR picked no Vulkan device: {error}"))?;
        let physical = vk::PhysicalDevice::from_raw(physical as u64);
        let hal = unsafe {
            wgpu::hal::vulkan::Instance::from_raw(
                entry,
                ash_instance,
                api,
                0,
                None,
                extensions,
                flags,
                wgpu::MemoryBudgetThresholds::default(),
                false,
                None,
            )
        }?;
        let exposed = hal
            .expose_adapter(physical)
            .ok_or("wgpu cannot use the Vulkan device OpenXR picked")?;
        shared.state().vulkan = Some(Vulkan {
            instance: handle,
            physical,
        });
        let instance = unsafe { wgpu::Instance::from_hal::<Api>(hal) };
        let adapter = unsafe { instance.create_adapter_from_hal(exposed) };
        Ok((instance, adapter))
    }

    fn device(
        &self,
        adapter: &wgpu::Adapter,
        extensions: &[&'static CStr],
        descriptor: &wgpu::DeviceDescriptor<'_>,
    ) -> Result<(wgpu::Device, wgpu::Queue), Box<dyn Error>> {
        let shared = &self.0;
        let (vk_instance, physical) = match &shared.state().vulkan {
            Some(vulkan) => (vulkan.instance, vulkan.physical),
            None => return Err("the adapter is not the one OpenXR picked".into()),
        };
        let hal =
            unsafe { adapter.as_hal::<Api>() }.ok_or("the adapter is not a Vulkan adapter")?;
        if hal.raw_physical_device() != physical {
            return Err("the adapter is not the one OpenXR picked".into());
        }
        let features = descriptor.required_features;
        let mut enabled = hal.required_device_extensions(features);
        let capabilities = hal.physical_device_capabilities();
        for &name in extensions {
            if capabilities.supports_extension(name) && !enabled.contains(&name) {
                enabled.push(name);
            }
        }
        let mut device_features = hal.physical_device_features(&enabled, features);
        let ash_instance = hal.shared_instance().raw_instance().clone();
        let entry = hal.shared_instance().entry().clone();
        let family = unsafe { ash_instance.get_physical_device_queue_family_properties(physical) }
            .iter()
            .position(|family| family.queue_flags.contains(vk::QueueFlags::GRAPHICS))
            .ok_or("the Vulkan device has no graphics queue")? as u32;
        let priorities = [1.0];
        let queues = [vk::DeviceQueueCreateInfo::default()
            .queue_family_index(family)
            .queue_priorities(&priorities)];
        let names: Vec<*const c_char> = enabled.iter().map(|name| name.as_ptr()).collect();
        let info = vk::DeviceCreateInfo::default()
            .queue_create_infos(&queues)
            .enabled_extension_names(&names);
        let info = device_features.add_to_device_create(info);
        let raw = unsafe {
            shared.instance.create_vulkan_device(
                shared.system,
                get_instance_proc_addr(&entry),
                physical.as_raw() as _,
                (&raw const info).cast(),
            )
        }
        .map_err(|error| format!("OpenXR could not create a Vulkan device: {error}"))?
        .map_err(|error| {
            format!(
                "OpenXR could not create a Vulkan device: {:?}",
                vk::Result::from_raw(error)
            )
        })?;
        let ash_device =
            unsafe { ash::Device::load(ash_instance.fp_v1_0(), vk::Device::from_raw(raw as u64)) };
        let destroyed = ash_device.clone();
        let owner = self.clone();
        let opened = unsafe {
            hal.device_from_raw(
                ash_device,
                Some(Box::new(move || {
                    let mut state = owner.0.state();
                    state.session = None;
                    state.device = None;
                    drop(state);
                    destroyed.destroy_device(None);
                })),
                &enabled,
                features,
                &descriptor.required_limits,
                &descriptor.memory_hints,
                family,
                0,
            )
        }?;
        drop(hal);
        let (device, queue) = unsafe { adapter.create_device_from_hal(opened, descriptor) }?;
        let (session, waiter, stream) = unsafe {
            shared.instance.create_session::<xr::Vulkan>(
                shared.system,
                &xr::vulkan::SessionCreateInfo {
                    instance: vk_instance.as_raw() as _,
                    physical_device: physical.as_raw() as _,
                    device: raw,
                    queue_family_index: family,
                    queue_index: 0,
                },
            )
        }
        .map_err(|error| format!("the OpenXR session did not start: {error}"))?;
        let mut state = shared.state();
        state.device = Some(Device {
            raw: vk::Device::from_raw(raw as u64),
            family,
        });
        state.session = Some(Session::new(
            &shared.instance,
            shared.system,
            session,
            waiter,
            stream,
        )?);
        drop(state);
        Ok((device, queue))
    }
}
