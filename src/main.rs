use ash::vk;
use ash::vk::Handle;
use std::ffi::CStr;
use std::ffi::CString;
use std::marker::PhantomData;
use std::ptr;

fn main() {
    unsafe {
        let sdl_context = sdl2::init().unwrap();
        let sdl_video = sdl_context.video().unwrap();
        let sdl_window = sdl_video
            .window("Window1", 800, 600)
            .position_centered()
            .vulkan()
            .build()
            .unwrap();

        let ash_entry = ash::Entry::load().unwrap();
        let vk_application_info = vk::ApplicationInfo {
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
        let vk_layers_raw = [CStr::from_bytes_with_nul(b"VK_LAYER_KHRONOS_validation\0")
            .unwrap()
            .as_ptr()];

        // For owning the null-terminated string
        let vk_enabled_extensions: Vec<_> = sdl_window
            .vulkan_instance_extensions()
            .unwrap()
            .iter()
            .map(|&s| CString::new(s).unwrap())
            .collect();
        let vk_instance_extensions_raw: Vec<_> =
            vk_enabled_extensions.iter().map(|s| s.as_ptr()).collect();

        let vk_instance_create_info = vk::InstanceCreateInfo {
            s_type: vk::StructureType::INSTANCE_CREATE_INFO,
            p_next: ptr::null(),
            flags: vk::InstanceCreateFlags::empty(),
            p_application_info: &vk_application_info,
            enabled_layer_count: vk_layers_raw.len() as u32,
            pp_enabled_layer_names: vk_layers_raw.as_ptr(),
            enabled_extension_count: vk_instance_extensions_raw.len() as u32,
            pp_enabled_extension_names: vk_instance_extensions_raw.as_ptr(),
            _marker: PhantomData,
        };
        let vk_instance = ash_entry
            .create_instance(&vk_instance_create_info, None)
            .unwrap();

        // Select ANY device, we don't care about any specific features
        let vk_physical_device_list = vk_instance.enumerate_physical_devices().unwrap();
        let mut vk_physical_device = vk::PhysicalDevice::null();
        let mut vk_graphics_queue_family_index = 0;
        'device_search_loop: for pd in vk_physical_device_list {
            let vk_queue_family_properties =
                vk_instance.get_physical_device_queue_family_properties(pd);
            for i in 0..vk_queue_family_properties.len() {
                let prop = vk_queue_family_properties[i];
                if prop.queue_flags.contains(vk::QueueFlags::GRAPHICS) {
                    vk_physical_device = pd;
                    vk_graphics_queue_family_index = i as u32;
                    break 'device_search_loop;
                }
            }
        }
        if vk_physical_device.is_null() {
            panic!("No fitting device found");
        }

        let vk_queue_priority = [1.0];
        let vk_device_queue_create_infos = [vk::DeviceQueueCreateInfo {
            s_type: vk::StructureType::DEVICE_QUEUE_CREATE_INFO,
            p_next: ptr::null(),
            flags: vk::DeviceQueueCreateFlags::empty(),
            queue_family_index: vk_graphics_queue_family_index,
            queue_count: 1,
            p_queue_priorities: vk_queue_priority.as_ptr(),
            _marker: PhantomData,
        }];
        let vk_device_extensions_raw = [CStr::from_bytes_with_nul(b"VK_KHR_swapchain\0").unwrap().as_ptr() ];
        let vk_device_create_info = vk::DeviceCreateInfo {
            s_type: vk::StructureType::DEVICE_CREATE_INFO,
            p_next: ptr::null(),
            flags: vk::DeviceCreateFlags::empty(),
            queue_create_info_count: 1,
            p_queue_create_infos: vk_device_queue_create_infos.as_ptr(),
            enabled_layer_count: 0,
            pp_enabled_layer_names: ptr::null(),
            enabled_extension_count: vk_device_extensions_raw.len() as u32,
            pp_enabled_extension_names: vk_device_extensions_raw.as_ptr(),
            p_enabled_features: ptr::null(),
            _marker: PhantomData,
        };
        let vk_device = vk_instance
            .create_device(vk_physical_device, &vk_device_create_info, None)
            .unwrap();

        let vk_graphics_queue = vk_device.get_device_queue(vk_graphics_queue_family_index, 0);

        vk_device.destroy_device(None);
        vk_instance.destroy_instance(None);
    }
}
