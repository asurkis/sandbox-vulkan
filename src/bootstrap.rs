use ash::vk;
use ash::vk::Handle;
use std::collections::HashSet;
use std::ffi::CStr;
use std::ffi::CString;
use std::ptr;

pub struct SdlBox {
    pub event_pump: sdl2::EventPump,
    pub window: sdl2::video::Window,
}

pub struct VkBox {
    pub instance: ash::Instance,
    pub instance_ext_surface: ash::khr::surface::Instance,
    pub physical_device_info: PhysicalDeviceInfo,
    pub device: ash::Device,
    pub device_ext_swapchain: ash::khr::swapchain::Device,
    pub queue_graphics: vk::Queue,
    pub queue_present: vk::Queue,
}

#[derive(Debug, Default, Clone)]
pub struct PhysicalDeviceInfo {
    pub surface: vk::SurfaceKHR,
    pub physical_device: vk::PhysicalDevice,
    pub queue_family_indices: [u32; 2],
    pub queue_family_index_graphics: u32,
    pub queue_family_index_present: u32,
    pub surface_capabilities: vk::SurfaceCapabilitiesKHR,
    pub surface_formats: Vec<vk::SurfaceFormatKHR>,
    pub surface_present_modes: Vec<vk::PresentModeKHR>,
}

impl SdlBox {
    pub fn new() -> Self {
        let system = sdl2::init().unwrap();
        let event_pump = system.event_pump().unwrap();
        let video = system.video().unwrap();
        let window = video
            .window("Window1", 1280, 720)
            .resizable()
            .position_centered()
            .vulkan()
            .build()
            .unwrap();
        Self { window, event_pump }
    }
}

impl VkBox {
    pub unsafe fn new(sdl: &SdlBox) -> Self {
        let ash_entry = ash::Entry::load().unwrap();
        let instance = Self::create_instance(&ash_entry, &sdl.window);
        let instance_ext_surface = ash::khr::surface::Instance::new(&ash_entry, &instance);
        let surface = sdl
            .window
            .vulkan_create_surface(instance.handle().as_raw() as _)
            .unwrap();
        let surface = vk::SurfaceKHR::from_raw(surface);
        let physical_device_info =
            PhysicalDeviceInfo::new(&instance, &instance_ext_surface, surface);
        let (device, [queue_graphics, queue_present]) = Self::create_device(
            &instance,
            physical_device_info.physical_device,
            [
                physical_device_info.queue_family_index_graphics,
                physical_device_info.queue_family_index_present,
            ],
        );
        let device_ext_swapchain = ash::khr::swapchain::Device::new(&instance, &device);
        Self {
            instance,
            instance_ext_surface,
            physical_device_info,
            device,
            device_ext_swapchain,
            queue_graphics,
            queue_present,
        }
    }

    pub unsafe fn create_semaphore(&self) -> vk::Semaphore {
        let create_info = vk::SemaphoreCreateInfo::default();
        self.device.create_semaphore(&create_info, None).unwrap()
    }

    pub unsafe fn create_fence(&self) -> vk::Fence {
        let create_info = vk::FenceCreateInfo::default();
        self.device.create_fence(&create_info, None).unwrap()
    }

    pub unsafe fn create_fence_signaled(&self) -> vk::Fence {
        let create_info = vk::FenceCreateInfo::default().flags(vk::FenceCreateFlags::SIGNALED);
        self.device.create_fence(&create_info, None).unwrap()
    }

    unsafe fn create_instance(
        ash_entry: &ash::Entry,
        window: &sdl2::video::Window,
    ) -> ash::Instance {
        let application_info = vk::ApplicationInfo::default()
            .application_name(CStr::from_bytes_with_nul(b"Sandbox App\0").unwrap())
            .application_version(0x0000_0001)
            .engine_name(CStr::from_bytes_with_nul(b"Sandbox Engine\0").unwrap())
            .engine_version(0x0000_0001)
            .api_version(vk::API_VERSION_1_1);
        let layers_raw = [CStr::from_bytes_with_nul(b"VK_LAYER_KHRONOS_validation\0")
            .unwrap()
            .as_ptr()];

        // For owning the null-terminated string
        let instance_extensions: Vec<_> = window
            .vulkan_instance_extensions()
            .unwrap()
            .iter()
            .map(|&s| CString::new(s).unwrap())
            .collect();
        let instance_extensions_raw: Vec<_> =
            instance_extensions.iter().map(|s| s.as_ptr()).collect();

        let create_info = vk::InstanceCreateInfo::default()
            .application_info(&application_info)
            .enabled_layer_names(&layers_raw)
            .enabled_extension_names(&instance_extensions_raw);
        ash_entry.create_instance(&create_info, None).unwrap()
    }

    unsafe fn create_device<const N: usize>(
        instance: &ash::Instance,
        physical_device: vk::PhysicalDevice,
        queue_family_indices: [u32; N],
    ) -> (ash::Device, [vk::Queue; N]) {
        let queue_priority = [1.0];
        let mut device_queue_create_infos = [vk::DeviceQueueCreateInfo::default(); N];
        for i in 0..N {
            device_queue_create_infos[i] = vk::DeviceQueueCreateInfo::default()
                .queue_family_index(queue_family_indices[i])
                .queue_priorities(&queue_priority);
        }
        let device_extensions_raw = [ash::khr::swapchain::NAME.as_ptr()];
        let device_create_info = vk::DeviceCreateInfo::default()
            .queue_create_infos(&device_queue_create_infos)
            .enabled_extension_names(&device_extensions_raw);
        let device = instance
            .create_device(physical_device, &device_create_info, None)
            .unwrap();
        let mut queues = [vk::Queue::default(); N];
        for i in 0..N {
            queues[i] = device.get_device_queue(queue_family_indices[i], 0);
        }
        (device, queues)
    }
}

impl Drop for VkBox {
    fn drop(&mut self) {
        unsafe {
            self.device.destroy_device(None);
            self.instance_ext_surface
                .destroy_surface(self.physical_device_info.surface, None);
            self.instance.destroy_instance(None);
        }
    }
}

impl PhysicalDeviceInfo {
    unsafe fn new(
        instance: &ash::Instance,
        instance_ext_surface: &ash::khr::surface::Instance,
        surface: vk::SurfaceKHR,
    ) -> Self {
        let physical_device_list = instance.enumerate_physical_devices().unwrap();
        for pd in physical_device_list {
            let mut info = Self::default();
            info.surface = surface;
            info.physical_device = pd;

            let mut required_extensions = HashSet::new();
            required_extensions.insert(ash::khr::swapchain::NAME);
            let extension_properties = instance.enumerate_device_extension_properties(pd).unwrap();
            for ext in extension_properties {
                required_extensions.remove(ext.extension_name_as_c_str().unwrap());
            }
            if !required_extensions.is_empty() {
                continue;
            }

            let queue_family_properties = instance.get_physical_device_queue_family_properties(pd);
            let mut has_graphics = false;
            let mut has_present = false;
            for i in 0..queue_family_properties.len() {
                let prop = queue_family_properties[i];
                if prop.queue_flags.contains(vk::QueueFlags::GRAPHICS) {
                    has_graphics = true;
                    info.queue_family_index_graphics = i as _;
                }
                if instance_ext_surface
                    .get_physical_device_surface_support(pd, i as _, surface)
                    .unwrap()
                {
                    has_present = true;
                    info.queue_family_index_present = i as _;
                }
            }

            if !has_graphics || !has_present {
                continue;
            }

            info.queue_family_indices = [
                info.queue_family_index_graphics,
                info.queue_family_index_present,
            ];

            info.surface_capabilities = instance_ext_surface
                .get_physical_device_surface_capabilities(pd, surface)
                .unwrap();
            info.surface_formats = instance_ext_surface
                .get_physical_device_surface_formats(pd, surface)
                .unwrap();
            info.surface_present_modes = instance_ext_surface
                .get_physical_device_surface_present_modes(pd, surface)
                .unwrap();

            if info.surface_formats.is_empty() || info.surface_present_modes.is_empty() {
                continue;
            }

            return info;
        }
        panic!("No fitting device found");
    }

    pub fn swapchain_create_info(&self) -> vk::SwapchainCreateInfoKHR {
        let mut swapchain_create_info = vk::SwapchainCreateInfoKHR::default()
            .surface(self.surface)
            .min_image_count(self.surface_capabilities.min_image_count)
            .image_format(self.surface_formats[0].format)
            .image_color_space(self.surface_formats[0].color_space)
            .image_extent(self.surface_capabilities.current_extent)
            .image_array_layers(1)
            .image_usage(vk::ImageUsageFlags::COLOR_ATTACHMENT)
            .image_sharing_mode(vk::SharingMode::CONCURRENT)
            .queue_family_indices(&self.queue_family_indices)
            .pre_transform(self.surface_capabilities.current_transform)
            .composite_alpha(vk::CompositeAlphaFlagsKHR::OPAQUE)
            .present_mode(vk::PresentModeKHR::FIFO)
            .clipped(false);
        if self.surface_capabilities.max_image_count == 0
            || self.surface_capabilities.min_image_count < self.surface_capabilities.max_image_count
        {
            swapchain_create_info.min_image_count += 1;
        }
        if self.queue_family_index_graphics == self.queue_family_index_present {
            swapchain_create_info.image_sharing_mode = vk::SharingMode::EXCLUSIVE;
            swapchain_create_info.queue_family_index_count = 0;
            swapchain_create_info.p_queue_family_indices = ptr::null();
        }
        for format in &self.surface_formats {
            if format.format == vk::Format::B8G8R8A8_UNORM
                && format.color_space == vk::ColorSpaceKHR::SRGB_NONLINEAR
            {
                swapchain_create_info.image_format = format.format;
                swapchain_create_info.image_color_space = format.color_space;
                break;
            }
        }
        for &present_mode in &self.surface_present_modes {
            if present_mode == vk::PresentModeKHR::MAILBOX {
                swapchain_create_info.present_mode = present_mode;
                break;
            }
        }
        swapchain_create_info
    }
}
