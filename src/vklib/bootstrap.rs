use super::{vkbox, CommittedImage, TransientGraphicsCommandBuffer};
use ash::vk::{self, Handle};
use std::{
    collections::HashSet,
    ffi::{CStr, CString},
};

pub struct SdlContext {
    pub event_pump: sdl2::EventPump,
    pub window: sdl2::video::Window,
}

#[derive(Clone)]
pub struct VkContext {
    pub instance: ash::Instance,
    pub instance_ext_surface: ash::khr::surface::Instance,
    pub surface: vk::SurfaceKHR,
    pub physical_device: PhysicalDeviceContext,
    pub device: ash::Device,
    pub device_ext_swapchain: ash::khr::swapchain::Device,
    pub queue_graphics: vk::Queue,
    pub queue_present: vk::Queue,
}

#[derive(Debug, Default, Clone)]
pub struct PhysicalDeviceContext {
    pub physical_device: vk::PhysicalDevice,
    pub queue_family_indices: [u32; 2],
    pub queue_family_index_graphics: u32,
    pub queue_family_index_present: u32,
    pub surface_formats: Vec<vk::SurfaceFormatKHR>,
    pub surface_present_modes: Vec<vk::PresentModeKHR>,
}

impl SdlContext {
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

impl VkContext {
    pub unsafe fn new(window: &sdl2::video::Window) -> Self {
        let ash_entry = ash::Entry::load().unwrap();
        let instance = Self::create_instance(&ash_entry, window);
        let instance_ext_surface = ash::khr::surface::Instance::new(&ash_entry, &instance);
        let surface = window
            .vulkan_create_surface(instance.handle().as_raw() as _)
            .unwrap();
        let surface = vk::SurfaceKHR::from_raw(surface);
        let physical_device = PhysicalDeviceContext::new(&instance, &instance_ext_surface, surface);
        let (device, [queue_graphics, queue_present]) = Self::create_device(
            &instance,
            physical_device.physical_device,
            [
                physical_device.queue_family_index_graphics,
                physical_device.queue_family_index_present,
            ],
        );
        let device_ext_swapchain = ash::khr::swapchain::Device::new(&instance, &device);
        Self {
            instance,
            instance_ext_surface,
            surface,
            physical_device,
            device,
            device_ext_swapchain,
            queue_graphics,
            queue_present,
        }
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
        let mut queue_create_infos = [vk::DeviceQueueCreateInfo::default(); N];
        for i in 0..N {
            queue_create_infos[i] = vk::DeviceQueueCreateInfo::default()
                .queue_family_index(queue_family_indices[i])
                .queue_priorities(&queue_priority);
        }
        let enabled_extension_names_raw = [ash::khr::swapchain::NAME.as_ptr()];
        let enabled_features = vk::PhysicalDeviceFeatures::default()
            .sampler_anisotropy(true)
            .fill_mode_non_solid(true);
        let device_create_info = vk::DeviceCreateInfo::default()
            .queue_create_infos(&queue_create_infos)
            .enabled_extension_names(&enabled_extension_names_raw)
            .enabled_features(&enabled_features);
        let device = instance
            .create_device(physical_device, &device_create_info, None)
            .unwrap();
        let mut queues = [vk::Queue::default(); N];
        for i in 0..N {
            queues[i] = device.get_device_queue(queue_family_indices[i], 0);
        }
        (device, queues)
    }

    pub unsafe fn select_image_format(
        &self,
        candidates: &[vk::Format],
        tiling: vk::ImageTiling,
        features: vk::FormatFeatureFlags,
    ) -> vk::Format {
        for &format in candidates {
            let props = self.instance.get_physical_device_format_properties(
                self.physical_device.physical_device,
                format,
            );
            let found_features = match tiling {
                vk::ImageTiling::LINEAR => props.linear_tiling_features,
                vk::ImageTiling::OPTIMAL => props.optimal_tiling_features,
                _ => panic!("Unexpected tiling: {tiling:?}"),
            };
            if found_features & features == features {
                return format;
            }
        }
        panic!("No fitting format found");
    }

    pub unsafe fn find_memory_type(
        &self,
        memory_requirements: vk::MemoryRequirements,
        memory_property_flags: vk::MemoryPropertyFlags,
    ) -> u32 {
        let memory_properties = self
            .instance
            .get_physical_device_memory_properties(self.physical_device.physical_device);

        for (i, memory_type) in memory_properties.memory_types_as_slice().iter().enumerate() {
            if memory_type.property_flags & memory_property_flags != memory_property_flags {
                continue;
            }
            if memory_requirements.memory_type_bits & (1 << i) != 0 {
                return i as _;
            }
        }
        panic!("No appropriate memory type found!");
    }

    pub unsafe fn allocate_memory(
        &self,
        memory_requirements: vk::MemoryRequirements,
        memory_property_flags: vk::MemoryPropertyFlags,
    ) -> vkbox::DeviceMemory {
        let memory_type_index = self.find_memory_type(memory_requirements, memory_property_flags);
        let allocate_info = vk::MemoryAllocateInfo::default()
            .allocation_size(memory_requirements.size)
            .memory_type_index(memory_type_index);
        vkbox::DeviceMemory::new(self, &allocate_info)
    }

    #[allow(unused)]
    pub unsafe fn create_fence(&self) -> vkbox::Fence {
        let create_info = vk::FenceCreateInfo::default();
        vkbox::Fence::new(self, &create_info)
    }

    pub unsafe fn create_fence_signaled(&self) -> vkbox::Fence {
        let create_info = vk::FenceCreateInfo::default().flags(vk::FenceCreateFlags::SIGNALED);
        vkbox::Fence::new(self, &create_info)
    }

    pub unsafe fn create_semaphore(&self) -> vkbox::Semaphore {
        let create_info = vk::SemaphoreCreateInfo::default();
        vkbox::Semaphore::new(self, &create_info)
    }

    #[allow(unused)]
    pub unsafe fn create_sampler(&self) -> vkbox::Sampler {
        let physical_device_properties = self
            .instance
            .get_physical_device_properties(self.physical_device.physical_device);
        let create_info = vk::SamplerCreateInfo::default()
            .mag_filter(vk::Filter::LINEAR)
            .min_filter(vk::Filter::LINEAR)
            .mipmap_mode(vk::SamplerMipmapMode::LINEAR)
            .address_mode_u(vk::SamplerAddressMode::REPEAT)
            .address_mode_v(vk::SamplerAddressMode::REPEAT)
            .address_mode_w(vk::SamplerAddressMode::REPEAT)
            .mip_lod_bias(0.0)
            .anisotropy_enable(true)
            .max_anisotropy(physical_device_properties.limits.max_sampler_anisotropy)
            .compare_enable(false)
            .compare_op(vk::CompareOp::NEVER)
            .min_lod(0.0)
            .max_lod(vk::LOD_CLAMP_NONE)
            .border_color(vk::BorderColor::INT_OPAQUE_BLACK)
            .unnormalized_coordinates(false);
        vkbox::Sampler::new(self, &create_info)
    }

    pub unsafe fn create_graphics_command_pool(&self) -> vkbox::CommandPool {
        let create_info = vk::CommandPoolCreateInfo::default()
            .flags(vk::CommandPoolCreateFlags::RESET_COMMAND_BUFFER)
            .queue_family_index(self.physical_device.queue_family_index_graphics);
        vkbox::CommandPool::new(self, &create_info)
    }

    pub unsafe fn create_graphics_transient_command_pool(&self) -> vkbox::CommandPool {
        let create_info = vk::CommandPoolCreateInfo::default()
            .flags(vk::CommandPoolCreateFlags::TRANSIENT)
            .queue_family_index(self.physical_device.queue_family_index_graphics);
        vkbox::CommandPool::new(self, &create_info)
    }

    pub unsafe fn create_shader_module(&self, bytecode: &[u8]) -> vkbox::ShaderModule {
        let mut code_safe = Vec::with_capacity((bytecode.len() + 3) / 4);
        for i in (0..bytecode.len()).step_by(4) {
            let mut arr = [0; 4];
            let end = bytecode.len().min(i + 4);
            arr[..end - i].copy_from_slice(&bytecode[i..end]);
            let u = u32::from_ne_bytes(arr);
            code_safe.push(u);
        }
        let create_info = vk::ShaderModuleCreateInfo::default().code(&code_safe);
        vkbox::ShaderModule::new(self, &create_info)
    }

    pub unsafe fn create_image_view(
        &self,
        image: vk::Image,
        format: vk::Format,
        aspect_mask: vk::ImageAspectFlags,
        mip_levels: u32,
    ) -> vkbox::ImageView {
        let create_info = vk::ImageViewCreateInfo::default()
            .image(image)
            .view_type(vk::ImageViewType::TYPE_2D)
            .format(format)
            .subresource_range(vk::ImageSubresourceRange {
                aspect_mask,
                base_mip_level: 0,
                level_count: mip_levels,
                base_array_layer: 0,
                layer_count: 1,
            });
        vkbox::ImageView::new(self, &create_info)
    }

    pub unsafe fn create_depth_buffer(
        &self,
        command_pool: vk::CommandPool,
        format: vk::Format,
        extent: vk::Extent2D,
        samples: vk::SampleCountFlags,
    ) -> CommittedImage {
        let depth_buffer = CommittedImage::new(
            self,
            format,
            extent,
            1,
            samples,
            vk::ImageUsageFlags::DEPTH_STENCIL_ATTACHMENT,
            vk::ImageAspectFlags::DEPTH,
        );
        let has_stencil_component =
            vk::Format::D32_SFLOAT_S8_UINT == format || vk::Format::D24_UNORM_S8_UINT == format;
        let mut aspect_mask = vk::ImageAspectFlags::DEPTH;
        if has_stencil_component {
            aspect_mask |= vk::ImageAspectFlags::STENCIL
        }

        let command_buffer = TransientGraphicsCommandBuffer::begin(self, command_pool);
        let image_memory_barriers = [vk::ImageMemoryBarrier::default()
            .src_access_mask(vk::AccessFlags::empty())
            .dst_access_mask(
                vk::AccessFlags::DEPTH_STENCIL_ATTACHMENT_READ
                    | vk::AccessFlags::DEPTH_STENCIL_ATTACHMENT_WRITE,
            )
            .old_layout(vk::ImageLayout::UNDEFINED)
            .new_layout(vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL)
            .src_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
            .dst_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
            .image(depth_buffer.image.0)
            .subresource_range(vk::ImageSubresourceRange {
                aspect_mask,
                base_mip_level: 0,
                level_count: 1,
                base_array_layer: 0,
                layer_count: 1,
            })];
        self.device.cmd_pipeline_barrier(
            command_buffer.buffer,
            vk::PipelineStageFlags::TOP_OF_PIPE,
            vk::PipelineStageFlags::EARLY_FRAGMENT_TESTS,
            vk::DependencyFlags::empty(),
            &[],
            &[],
            &image_memory_barriers,
        );

        depth_buffer
    }

    pub unsafe fn allocate_command_buffers(
        &self,
        command_pool: vk::CommandPool,
        count: u32,
    ) -> Vec<vk::CommandBuffer> {
        let allocate_info = vk::CommandBufferAllocateInfo::default()
            .command_pool(command_pool)
            .level(vk::CommandBufferLevel::PRIMARY)
            .command_buffer_count(count);
        self.device
            .allocate_command_buffers(&allocate_info)
            .unwrap()
    }
}

impl Drop for VkContext {
    fn drop(&mut self) {
        unsafe {
            self.device.destroy_device(None);
            self.instance_ext_surface
                .destroy_surface(self.surface, None);
            self.instance.destroy_instance(None);
        }
    }
}

impl PhysicalDeviceContext {
    unsafe fn new(
        instance: &ash::Instance,
        instance_ext_surface: &ash::khr::surface::Instance,
        surface: vk::SurfaceKHR,
    ) -> Self {
        let physical_device_list = instance.enumerate_physical_devices().unwrap();
        for pd in physical_device_list {
            let mut info = Self {
                physical_device: pd,
                ..Default::default()
            };

            let device_features = instance.get_physical_device_features(pd);
            if device_features.sampler_anisotropy == 0 {
                continue;
            }

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
            for (i, prop) in queue_family_properties.iter().enumerate() {
                let graphics_flags =
                    vk::QueueFlags::GRAPHICS | vk::QueueFlags::COMPUTE | vk::QueueFlags::TRANSFER;
                if prop.queue_flags & graphics_flags == graphics_flags {
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
}
