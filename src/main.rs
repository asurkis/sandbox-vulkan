use ash::vk;
use ash::vk::Handle;
use std::collections::HashSet;
use std::ffi::CStr;
use std::ffi::CString;
use std::marker::PhantomData;
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

        let physical_device_info = pick_physical_device(&instance, &instance_ext_surface, surface);
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

        window.vulkan_drawable_size();

        create_swapchain(
            &device_ext_swapchain,
            surface,
            &physical_device_info,
        );

        device.destroy_device(None);
        instance_ext_surface.destroy_surface(surface, None);
        instance.destroy_instance(None);
    }
}

unsafe fn create_instance(ash_entry: &ash::Entry, window: &sdl2::video::Window) -> ash::Instance {
    let application_info = vk::ApplicationInfo {
        s_type: vk::StructureType::APPLICATION_INFO,
        p_next: ptr::null(),
        p_application_name: CStr::from_bytes_with_nul(b"Sandbox App\0")
            .unwrap()
            .as_ptr(),
        application_version: 0x00000001,
        p_engine_name: CStr::from_bytes_with_nul(b"Sandbox Engine\0")
            .unwrap()
            .as_ptr(),
        engine_version: 0x00000001,
        api_version: vk::API_VERSION_1_1,
        _marker: PhantomData,
    };
    let layers_raw = [CStr::from_bytes_with_nul(b"VK_LAYER_KHRONOS_validation\0")
        .unwrap()
        .as_ptr()];

    // For owning the null-terminated string
    let vk_instance_extensions: Vec<_> = window
        .vulkan_instance_extensions()
        .unwrap()
        .iter()
        .map(|&s| CString::new(s).unwrap())
        .collect();
    let vk_instance_extensions_raw: Vec<_> =
        vk_instance_extensions.iter().map(|s| s.as_ptr()).collect();

    let vk_instance_create_info = vk::InstanceCreateInfo {
        s_type: vk::StructureType::INSTANCE_CREATE_INFO,
        p_next: ptr::null(),
        flags: vk::InstanceCreateFlags::empty(),
        p_application_info: &application_info,
        enabled_layer_count: layers_raw.len() as u32,
        pp_enabled_layer_names: layers_raw.as_ptr(),
        enabled_extension_count: vk_instance_extensions_raw.len() as u32,
        pp_enabled_extension_names: vk_instance_extensions_raw.as_ptr(),
        _marker: PhantomData,
    };
    ash_entry
        .create_instance(&vk_instance_create_info, None)
        .unwrap()
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

unsafe fn pick_physical_device(
    instance: &ash::Instance,
    instance_ext_surface: &ash::khr::surface::Instance,
    surface: vk::SurfaceKHR,
) -> PhysicalDeviceInfo {
    let physical_device_list = instance.enumerate_physical_devices().unwrap();
    for pd in physical_device_list {
        let mut info = PhysicalDeviceInfo::default();
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

unsafe fn create_device<const N: usize>(
    instance: &ash::Instance,
    physical_device: vk::PhysicalDevice,
    queue_family_indices: [u32; N],
) -> (ash::Device, [vk::Queue; N]) {
    let queue_priority = [1.0];
    let mut device_queue_create_infos = [vk::DeviceQueueCreateInfo::default(); N];
    for i in 0..N {
        device_queue_create_infos[i] = vk::DeviceQueueCreateInfo {
            s_type: vk::StructureType::DEVICE_QUEUE_CREATE_INFO,
            p_next: ptr::null(),
            flags: vk::DeviceQueueCreateFlags::empty(),
            queue_family_index: queue_family_indices[i],
            queue_count: 1,
            p_queue_priorities: queue_priority.as_ptr(),
            _marker: PhantomData,
        };
    }
    let device_extensions_raw = [ash::khr::swapchain::NAME.as_ptr()];
    #[allow(deprecated)]
    let device_create_info = vk::DeviceCreateInfo {
        s_type: vk::StructureType::DEVICE_CREATE_INFO,
        p_next: ptr::null(),
        flags: vk::DeviceCreateFlags::empty(),
        queue_create_info_count: 1,
        p_queue_create_infos: device_queue_create_infos.as_ptr(),
        enabled_layer_count: 0,              // deprecated, unused
        pp_enabled_layer_names: ptr::null(), // deprecated, unused
        enabled_extension_count: device_extensions_raw.len() as u32,
        pp_enabled_extension_names: device_extensions_raw.as_ptr(),
        p_enabled_features: ptr::null(),
        _marker: PhantomData,
    };
    let device = instance
        .create_device(physical_device, &device_create_info, None)
        .unwrap();

    let mut queues = [vk::Queue::default(); N];
    for i in 0..N {
        queues[i] = device.get_device_queue(queue_family_indices[i], 0);
    }

    (device, queues)
}

unsafe fn create_swapchain(
    device_ext_swapchain: &ash::khr::swapchain::Device,
    surface: vk::SurfaceKHR,
    pdi: &PhysicalDeviceInfo,
) -> vk::SwapchainKHR {
    let queue_family_indices = [
        pdi.queue_family_index_graphics,
        pdi.queue_family_index_present,
    ];
    let mut swapchain_create_info = vk::SwapchainCreateInfoKHR {
        s_type: vk::StructureType::SWAPCHAIN_CREATE_INFO_KHR,
        p_next: ptr::null(),
        flags: vk::SwapchainCreateFlagsKHR::empty(),
        surface,
        min_image_count: pdi.surface_capabilities.min_image_count,
        image_format: pdi.surface_formats[0].format,
        image_color_space: pdi.surface_formats[0].color_space,
        image_extent: pdi.surface_capabilities.current_extent,
        image_array_layers: 1,
        image_usage: vk::ImageUsageFlags::COLOR_ATTACHMENT,
        image_sharing_mode: vk::SharingMode::CONCURRENT,
        queue_family_index_count: 2,
        p_queue_family_indices: queue_family_indices.as_ptr(),
        pre_transform: pdi.surface_capabilities.current_transform,
        composite_alpha: vk::CompositeAlphaFlagsKHR::OPAQUE,
        present_mode: vk::PresentModeKHR::FIFO,
        clipped: vk::FALSE,
        old_swapchain: vk::SwapchainKHR::null(),
        _marker: PhantomData,
    };
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
    device_ext_swapchain
        .create_swapchain(&swapchain_create_info, None)
        .unwrap()
}
