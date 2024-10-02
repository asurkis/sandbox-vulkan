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
        let vk_instance = ash_entry
            .create_instance(&vk_instance_create_info, None)
            .unwrap();
        let vk_instance_ext_surface = ash::khr::surface::Instance::new(&ash_entry, &vk_instance);

        let surface_raw = window
            .vulkan_create_surface(vk_instance.handle().as_raw() as usize)
            .unwrap();
        let surface = vk::SurfaceKHR::from_raw(surface_raw);

        let physical_device_list = vk_instance.enumerate_physical_devices().unwrap();
        let mut physical_device = vk::PhysicalDevice::null();
        let mut queue_family_index_graphics = 0;
        let mut queue_family_index_present = 0;
        for pd in physical_device_list {
            let mut required_extensions = HashSet::new();
            required_extensions.insert(ash::khr::swapchain::NAME);
            let extension_properties = vk_instance
                .enumerate_device_extension_properties(pd)
                .unwrap();
            for ext in extension_properties {
                required_extensions.remove(ext.extension_name_as_c_str().unwrap());
            }
            if !required_extensions.is_empty() {
                continue;
            }

            let queue_family_properties =
                vk_instance.get_physical_device_queue_family_properties(pd);
            let mut has_graphics = false;
            let mut has_present = false;
            for i in 0..queue_family_properties.len() {
                let prop = queue_family_properties[i];
                if prop.queue_flags.contains(vk::QueueFlags::GRAPHICS) {
                    has_graphics = true;
                    queue_family_index_graphics = i as u32;
                }
                if vk_instance_ext_surface
                    .get_physical_device_surface_support(pd, i as u32, surface)
                    .unwrap()
                {
                    has_present = true;
                    queue_family_index_present = i as u32;
                }
            }

            let surface_capabilities = vk_instance_ext_surface
                .get_physical_device_surface_capabilities(pd, surface)
                .unwrap();
            let surface_formats = vk_instance_ext_surface
                .get_physical_device_surface_formats(pd, surface)
                .unwrap();
            let surface_present_modes = vk_instance_ext_surface
                .get_physical_device_surface_present_modes(pd, surface)
                .unwrap();

            if surface_formats.is_empty() || surface_present_modes.is_empty() {
                continue;
            }

            if has_graphics && has_present {
                physical_device = pd;
                break;
            }
        }
        if physical_device.is_null() {
            panic!("No fitting device found");
        }

        let queue_priority = [1.0];
        let device_queue_create_infos = [
            vk::DeviceQueueCreateInfo {
                s_type: vk::StructureType::DEVICE_QUEUE_CREATE_INFO,
                p_next: ptr::null(),
                flags: vk::DeviceQueueCreateFlags::empty(),
                queue_family_index: queue_family_index_graphics,
                queue_count: 1,
                p_queue_priorities: queue_priority.as_ptr(),
                _marker: PhantomData,
            },
            vk::DeviceQueueCreateInfo {
                s_type: vk::StructureType::DEVICE_QUEUE_CREATE_INFO,
                p_next: ptr::null(),
                flags: vk::DeviceQueueCreateFlags::empty(),
                queue_family_index: queue_family_index_present,
                queue_count: 1,
                p_queue_priorities: queue_priority.as_ptr(),
                _marker: PhantomData,
            },
        ];
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
        let device = vk_instance
            .create_device(physical_device, &device_create_info, None)
            .unwrap();
        let device_ext_swapchain = ash::khr::swapchain::Device::new(&vk_instance, &device);

        let queue_graphics = device.get_device_queue(queue_family_index_graphics, 0);
        let queue_present = device.get_device_queue(queue_family_index_present, 0);

        // let swapchain_create_info = vk::SwapchainCreateInfoKHR {
        //     s_type: vk::StructureType::SWAPCHAIN_CREATE_INFO_KHR,
        //     p_next: ptr::null(),
        //     flags: vk::SwapchainCreateFlagsKHR::empty(),
        //     surface,
        //     min_image_count: 3,
        //     image_format: vk::Format::R8G8B8A8_UNORM,
        //     image_color_space: vk::ColorSpaceKHR::SRGB_NONLINEAR,
        //     image_extent: vk::Extent2D {
        //         width: 0,
        //         height: 0,
        //     },
        //     image_array_layers: 1,
        //     image_usage: vk::ImageUsageFlags::COLOR_ATTACHMENT,
        //     image_sharing_mode: if queue_graphics == queue_present {
        //         vk::SharingMode::EXCLUSIVE
        //     } else {
        //         vk::SharingMode::CONCURRENT
        //     },
        //     queue_family_index_count: if queue_graphics == queue_present { 0 } else { 2} ,
        //     p_queue_family_indices: if queue_graphics == queue_present { ptr::null() } else { ptr::null()},
        //     pre_transform: vk::SurfaceTransformFlagsKHR::empty(),

        // };

        device.destroy_device(None);
        vk_instance_ext_surface.destroy_surface(surface, None);
        vk_instance.destroy_instance(None);
    }
}
