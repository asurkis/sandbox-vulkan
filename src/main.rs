use ash::vk;
use ash::vk::Handle;
use std::collections::HashSet;
use std::ffi::CStr;
use std::ffi::CString;
use std::ptr;

fn main() {
    unsafe {
        let sdl_context = sdl2::init().unwrap();
        let sdl_video = sdl_context.video().unwrap();
        let window = sdl_video
            .window("Window1", 1280, 720)
            .position_centered()
            .vulkan()
            .build()
            .unwrap();

        let ash_entry = ash::Entry::load().unwrap();
        let instance = create_instance(&ash_entry, &window);
        let instance_ext_surface = ash::khr::surface::Instance::new(&ash_entry, &instance);

        let surface_raw = window
            .vulkan_create_surface(instance.handle().as_raw() as usize)
            .unwrap();
        let surface = vk::SurfaceKHR::from_raw(surface_raw);

        let physical_device_info =
            PhysicalDeviceInfo::new(&instance, &instance_ext_surface, surface);
        let physical_device = physical_device_info.physical_device;

        let (device, [queue_graphics, queue_present]) = create_device(
            &instance,
            physical_device,
            [
                physical_device_info.queue_family_index_graphics,
                physical_device_info.queue_family_index_present,
            ],
        );
        let device_ext_swapchain = ash::khr::swapchain::Device::new(&instance, &device);

        let swapchain_info =
            SwapchainInfo::new(&device_ext_swapchain, surface, &physical_device_info);
        let swapchain_image_views =
            create_image_views(&device, &swapchain_info.images, swapchain_info.image_format);

        for view in swapchain_image_views {
            device.destroy_image_view(view, None);
        }
        device_ext_swapchain.destroy_swapchain(swapchain_info.swapchain, None);
        device.destroy_device(None);
        instance_ext_surface.destroy_surface(surface, None);
        instance.destroy_instance(None);
    }
}

unsafe fn create_instance(ash_entry: &ash::Entry, window: &sdl2::video::Window) -> ash::Instance {
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
    let instance_extensions_raw: Vec<_> = instance_extensions.iter().map(|s| s.as_ptr()).collect();

    let create_info = vk::InstanceCreateInfo::default()
        .application_info(&application_info)
        .enabled_layer_names(&layers_raw)
        .enabled_extension_names(&instance_extensions_raw);
    ash_entry.create_instance(&create_info, None).unwrap()
}

#[derive(Debug, Default, Clone)]
struct PhysicalDeviceInfo {
    physical_device: vk::PhysicalDevice,
    queue_family_index_graphics: u32,
    queue_family_index_present: u32,
    surface_capabilities: vk::SurfaceCapabilitiesKHR,
    surface_formats: Vec<vk::SurfaceFormatKHR>,
    surface_present_modes: Vec<vk::PresentModeKHR>,
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
                    info.queue_family_index_graphics = i as u32;
                }
                if instance_ext_surface
                    .get_physical_device_surface_support(pd, i as u32, surface)
                    .unwrap()
                {
                    has_present = true;
                    info.queue_family_index_present = i as u32;
                }
            }

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

            if has_graphics && has_present {
                return info;
            }
        }
        panic!("No fitting device found");
    }
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

#[derive(Debug, Default, Clone)]
struct SwapchainInfo {
    swapchain: vk::SwapchainKHR,
    image_format: vk::Format,
    images: Vec<vk::Image>,
}

impl SwapchainInfo {
    unsafe fn new(
        device_ext_swapchain: &ash::khr::swapchain::Device,
        surface: vk::SurfaceKHR,
        pdi: &PhysicalDeviceInfo,
    ) -> Self {
        let queue_family_indices = [
            pdi.queue_family_index_graphics,
            pdi.queue_family_index_present,
        ];
        let mut swapchain_create_info = vk::SwapchainCreateInfoKHR::default()
            .surface(surface)
            .min_image_count(pdi.surface_capabilities.min_image_count)
            .image_format(pdi.surface_formats[0].format)
            .image_color_space(pdi.surface_formats[0].color_space)
            .image_extent(pdi.surface_capabilities.current_extent)
            .image_array_layers(1)
            .image_usage(vk::ImageUsageFlags::COLOR_ATTACHMENT)
            .image_sharing_mode(vk::SharingMode::CONCURRENT)
            .queue_family_indices(&queue_family_indices)
            .pre_transform(pdi.surface_capabilities.current_transform)
            .composite_alpha(vk::CompositeAlphaFlagsKHR::OPAQUE)
            .present_mode(vk::PresentModeKHR::FIFO)
            .clipped(false);
        if pdi.surface_capabilities.max_image_count == 0
            || pdi.surface_capabilities.min_image_count < pdi.surface_capabilities.max_image_count
        {
            swapchain_create_info.min_image_count += 1;
        }
        if pdi.queue_family_index_graphics == pdi.queue_family_index_present {
            swapchain_create_info.image_sharing_mode = vk::SharingMode::EXCLUSIVE;
            swapchain_create_info.queue_family_index_count = 0;
            swapchain_create_info.p_queue_family_indices = ptr::null();
        }
        for format in &pdi.surface_formats {
            if format.format == vk::Format::B8G8R8A8_UNORM
                && format.color_space == vk::ColorSpaceKHR::SRGB_NONLINEAR
            {
                swapchain_create_info.image_format = format.format;
                swapchain_create_info.image_color_space = format.color_space;
                break;
            }
        }
        for &present_mode in &pdi.surface_present_modes {
            if present_mode == vk::PresentModeKHR::MAILBOX {
                swapchain_create_info.present_mode = present_mode;
                break;
            }
        }

        let swapchain = device_ext_swapchain
            .create_swapchain(&swapchain_create_info, None)
            .unwrap();
        let images = device_ext_swapchain
            .get_swapchain_images(swapchain)
            .unwrap();
        Self {
            swapchain,
            image_format: swapchain_create_info.image_format,
            images,
        }
    }
}

unsafe fn create_image_views(
    device: &ash::Device,
    images: &[vk::Image],
    format: vk::Format,
) -> Vec<vk::ImageView> {
    let mut views = Vec::with_capacity(images.len());
    for &image in images {
        let view_create_info = vk::ImageViewCreateInfo::default()
            .image(image)
            .view_type(vk::ImageViewType::TYPE_2D)
            .format(format)
            .subresource_range(vk::ImageSubresourceRange {
                aspect_mask: vk::ImageAspectFlags::COLOR,
                base_mip_level: 0,
                level_count: 1,
                base_array_layer: 0,
                layer_count: 1,
            });
        let view = device.create_image_view(&view_create_info, None).unwrap();
        views.push(view);
    }
    views
}
