use crate::vkbox;
use ash::vk;
use ash::vk::Handle;
use std::collections::HashSet;
use std::ffi::CStr;
use std::ffi::CString;
use std::mem;
use std::ptr;

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

pub struct Swapchain<'a> {
    pub framebuffers: Vec<vkbox::Framebuffer<'a>>,
    pub _color_buffer: CommittedImage<'a>,
    pub _depth_buffer: CommittedImage<'a>,
    pub _image_views: Vec<vkbox::ImageView<'a>>,
    pub swapchain: vkbox::SwapchainKHR<'a>,

    command_pool: vk::CommandPool,
    render_pass: vk::RenderPass,
    pub extent: vk::Extent2D,
    depth_buffer_format: vk::Format,
    samples: vk::SampleCountFlags,

    vk: &'a VkContext,
}

pub struct TransientGraphicsCommandBuffer<'a> {
    pub buffer: vk::CommandBuffer,
    pub pool: vk::CommandPool,
    vk: &'a VkContext,
}

#[derive(Debug, Default)]
pub struct CommittedBuffer<'a> {
    pub buffer: vkbox::Buffer<'a>,
    pub memory: vkbox::DeviceMemory<'a>,
}

#[derive(Debug, Default)]
pub struct CommittedImage<'a> {
    pub image: vkbox::Image<'a>,
    pub view: vkbox::ImageView<'a>,
    pub _memory: vkbox::DeviceMemory<'a>,
}

impl SdlContext {
    pub fn new() -> Self {
        let system = sdl2::init().unwrap();
        let event_pump = system.event_pump().unwrap();
        let video = system.video().unwrap();
        let window = video
            .window("Window1", 900, 900)
            .resizable()
            .position_centered()
            .vulkan()
            .build()
            .unwrap();
        Self { window, event_pump }
    }
}

impl VkContext {
    pub unsafe fn new(sdl: &SdlContext) -> Self {
        let ash_entry = ash::Entry::load().unwrap();
        let instance = Self::create_instance(&ash_entry, &sdl.window);
        let instance_ext_surface = ash::khr::surface::Instance::new(&ash_entry, &instance);
        let surface = sdl
            .window
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
        let enabled_features = vk::PhysicalDeviceFeatures::default().sampler_anisotropy(true);
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
        vkbox::DeviceMemory::create(self, &allocate_info)
    }

    #[allow(unused)]
    pub unsafe fn create_fence(&self) -> vkbox::Fence {
        let create_info = vk::FenceCreateInfo::default();
        vkbox::Fence::create(self, &create_info)
    }

    pub unsafe fn create_fence_signaled(&self) -> vkbox::Fence {
        let create_info = vk::FenceCreateInfo::default().flags(vk::FenceCreateFlags::SIGNALED);
        vkbox::Fence::create(self, &create_info)
    }

    pub unsafe fn create_semaphore(&self) -> vkbox::Semaphore {
        let create_info = vk::SemaphoreCreateInfo::default();
        vkbox::Semaphore::create(self, &create_info)
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
        vkbox::Sampler::create(self, &create_info)
    }

    pub unsafe fn create_graphics_command_pool(&self) -> vkbox::CommandPool {
        let create_info = vk::CommandPoolCreateInfo::default()
            .flags(vk::CommandPoolCreateFlags::RESET_COMMAND_BUFFER)
            .queue_family_index(self.physical_device.queue_family_index_graphics);
        vkbox::CommandPool::create(self, &create_info)
    }

    pub unsafe fn create_graphics_transient_command_pool(&self) -> vkbox::CommandPool {
        let create_info = vk::CommandPoolCreateInfo::default()
            .flags(vk::CommandPoolCreateFlags::TRANSIENT)
            .queue_family_index(self.physical_device.queue_family_index_graphics);
        vkbox::CommandPool::create(self, &create_info)
    }

    pub unsafe fn create_shader_module(&self, bytecode: &[u8]) -> vkbox::ShaderModule {
        let mut code_safe = Vec::with_capacity((bytecode.len() + 3) / 4);
        for i in (0..bytecode.len()).step_by(4) {
            let mut arr = [0; 4];
            for j in i..bytecode.len().min(i + 4) {
                arr[j - i] = bytecode[j];
            }
            let u = u32::from_ne_bytes(arr);
            code_safe.push(u);
        }
        let create_info = vk::ShaderModuleCreateInfo::default().code(&code_safe);
        vkbox::ShaderModule::create(self, &create_info)
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
        vkbox::ImageView::create(self, &create_info)
    }

    pub unsafe fn create_depth_buffer(
        &self,
        command_pool: vk::CommandPool,
        format: vk::Format,
        extent: vk::Extent2D,
        samples: vk::SampleCountFlags,
    ) -> CommittedImage {
        let depth_buffer = CommittedImage::create(
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
            let mut info = Self::default();
            info.physical_device = pd;

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
            for i in 0..queue_family_properties.len() {
                let prop = queue_family_properties[i];
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

impl<'a> Swapchain<'a> {
    pub unsafe fn new(
        vk: &'a VkContext,
        command_pool: vk::CommandPool,
        render_pass: vk::RenderPass,
        depth_buffer_format: vk::Format,
        samples: vk::SampleCountFlags,
        old_swapchain: Option<&Self>,
    ) -> Self {
        let mut create_info = Self::create_info(vk);
        if let Some(old) = old_swapchain {
            create_info.old_swapchain = old.swapchain.0;
            vk.device.device_wait_idle().unwrap();
        }
        let swapchain = vkbox::SwapchainKHR::create(vk, &create_info);
        let images = vk
            .device_ext_swapchain
            .get_swapchain_images(swapchain.0)
            .unwrap();
        let image_views: Vec<_> = images
            .iter()
            .map(|&img| {
                vk.create_image_view(
                    img,
                    create_info.image_format,
                    vk::ImageAspectFlags::COLOR,
                    1,
                )
            })
            .collect();
        let msaa_on = samples != vk::SampleCountFlags::TYPE_1;
        let color_buffer = if msaa_on {
            CommittedImage::create(
                vk,
                create_info.image_format,
                create_info.image_extent,
                1,
                samples,
                vk::ImageUsageFlags::COLOR_ATTACHMENT,
                vk::ImageAspectFlags::COLOR,
            )
        } else {
            CommittedImage::default()
        };
        let depth_buffer = vk.create_depth_buffer(
            command_pool,
            depth_buffer_format,
            create_info.image_extent,
            samples,
        );
        let framebuffers: Vec<_> = image_views
            .iter()
            .map(|iv| {
                let attachments = if msaa_on {
                    vec![color_buffer.view.0, depth_buffer.view.0, iv.0]
                } else {
                    vec![iv.0, depth_buffer.view.0]
                };
                let extent = create_info.image_extent;
                let create_info = vk::FramebufferCreateInfo::default()
                    .render_pass(render_pass)
                    .attachments(&attachments)
                    .width(extent.width)
                    .height(extent.height)
                    .layers(1);
                vkbox::Framebuffer::create(vk, &create_info)
            })
            .collect();
        Self {
            framebuffers,
            _color_buffer: color_buffer,
            _depth_buffer: depth_buffer,
            _image_views: image_views,
            swapchain,
            command_pool,
            render_pass,
            extent: create_info.image_extent,
            depth_buffer_format,
            samples,
            vk,
        }
    }

    unsafe fn create_info(vk: &'a VkContext) -> vk::SwapchainCreateInfoKHR {
        let surface_capabilities = vk
            .instance_ext_surface
            .get_physical_device_surface_capabilities(
                vk.physical_device.physical_device,
                vk.surface,
            )
            .unwrap();
        let mut swapchain_create_info = vk::SwapchainCreateInfoKHR::default()
            .surface(vk.surface)
            .min_image_count(surface_capabilities.min_image_count)
            .image_format(vk.physical_device.surface_formats[0].format)
            .image_color_space(vk.physical_device.surface_formats[0].color_space)
            .image_extent(surface_capabilities.current_extent)
            .image_array_layers(1)
            .image_usage(vk::ImageUsageFlags::COLOR_ATTACHMENT)
            .image_sharing_mode(vk::SharingMode::CONCURRENT)
            .queue_family_indices(&vk.physical_device.queue_family_indices)
            .pre_transform(surface_capabilities.current_transform)
            .composite_alpha(vk::CompositeAlphaFlagsKHR::OPAQUE)
            .present_mode(vk::PresentModeKHR::FIFO)
            .clipped(false);
        if surface_capabilities.max_image_count == 0
            || surface_capabilities.min_image_count < surface_capabilities.max_image_count
        {
            swapchain_create_info.min_image_count += 1;
        }
        if vk.physical_device.queue_family_index_graphics
            == vk.physical_device.queue_family_index_present
        {
            swapchain_create_info.image_sharing_mode = vk::SharingMode::EXCLUSIVE;
            swapchain_create_info.queue_family_index_count = 0;
            swapchain_create_info.p_queue_family_indices = ptr::null();
        }
        for format in &vk.physical_device.surface_formats {
            if format.format == vk::Format::B8G8R8A8_UNORM
                && format.color_space == vk::ColorSpaceKHR::SRGB_NONLINEAR
            {
                swapchain_create_info.image_format = format.format;
                swapchain_create_info.image_color_space = format.color_space;
                break;
            }
        }
        for &present_mode in &vk.physical_device.surface_present_modes {
            if present_mode == vk::PresentModeKHR::MAILBOX {
                swapchain_create_info.present_mode = present_mode;
                break;
            }
        }
        swapchain_create_info
    }

    pub unsafe fn reinit(&mut self) {
        let capabilities = self
            .vk
            .instance_ext_surface
            .get_physical_device_surface_capabilities(
                self.vk.physical_device.physical_device,
                self.vk.surface,
            )
            .unwrap();
        let extent = capabilities.current_extent;
        if extent.width == 0 || extent.height == 0 {
            return;
        }
        let mut new = Self::new(
            self.vk,
            self.command_pool,
            self.render_pass,
            self.depth_buffer_format,
            self.samples,
            Some(self),
        );
        mem::swap(self, &mut new);
    }
}

impl<'a> TransientGraphicsCommandBuffer<'a> {
    pub unsafe fn begin(vk: &'a VkContext, pool: vk::CommandPool) -> Self {
        let buffer = vk.allocate_command_buffers(pool, 1)[0];
        let begin_info = vk::CommandBufferBeginInfo::default()
            .flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT);
        vk.device.begin_command_buffer(buffer, &begin_info).unwrap();
        Self { buffer, pool, vk }
    }
}

impl<'a> Drop for TransientGraphicsCommandBuffer<'a> {
    fn drop(&mut self) {
        unsafe {
            self.vk.device.end_command_buffer(self.buffer).unwrap();

            let buffers = [self.buffer];
            let submits = [vk::SubmitInfo::default().command_buffers(&buffers)];
            self.vk
                .device
                .queue_submit(self.vk.queue_graphics, &submits, vk::Fence::null())
                .unwrap();
            self.vk
                .device
                .queue_wait_idle(self.vk.queue_graphics)
                .unwrap();
            self.vk.device.free_command_buffers(self.pool, &buffers);
        }
    }
}

impl<'a> CommittedBuffer<'a> {
    pub unsafe fn create(
        vk: &'a VkContext,
        size: u64,
        usage: vk::BufferUsageFlags,
        memory_property_flags: vk::MemoryPropertyFlags,
    ) -> Self {
        let queue_family_indices = [vk.physical_device.queue_family_index_graphics];
        let create_info = vk::BufferCreateInfo::default()
            .size(size)
            .usage(usage)
            .sharing_mode(vk::SharingMode::EXCLUSIVE)
            .queue_family_indices(&queue_family_indices);
        let buffer = vkbox::Buffer::create(vk, &create_info);

        let memory_requirements = vk.device.get_buffer_memory_requirements(buffer.0);
        let memory = vk.allocate_memory(memory_requirements, memory_property_flags);
        vk.device.bind_buffer_memory(buffer.0, memory.0, 0).unwrap();
        Self { buffer, memory }
    }

    pub unsafe fn new_staging<T: Copy>(vk: &'a VkContext, data: &[T]) -> Self {
        let data_size = mem::size_of_val(data);
        let staging = Self::create(
            vk,
            data_size as _,
            vk::BufferUsageFlags::TRANSFER_SRC,
            vk::MemoryPropertyFlags::HOST_VISIBLE | vk::MemoryPropertyFlags::HOST_COHERENT,
        );
        let memmap = vk
            .device
            .map_memory(
                staging.memory.0,
                0,
                data_size as _,
                vk::MemoryMapFlags::empty(),
            )
            .unwrap();
        ptr::copy(mem::transmute(data.as_ptr()), memmap, data_size);
        vk.device.unmap_memory(staging.memory.0);
        staging
    }

    pub unsafe fn upload<T: Copy>(
        vk: &'a VkContext,
        command_pool: vk::CommandPool,
        data: &[T],
        usage: vk::BufferUsageFlags,
    ) -> Self {
        let data_size = mem::size_of_val(data);
        let out = Self::create(
            vk,
            data_size as _,
            usage | vk::BufferUsageFlags::TRANSFER_DST,
            vk::MemoryPropertyFlags::DEVICE_LOCAL,
        );
        let staging = Self::new_staging(vk, data);

        let command_buffer = TransientGraphicsCommandBuffer::begin(vk, command_pool);
        let regions = [vk::BufferCopy {
            src_offset: 0,
            dst_offset: 0,
            size: data_size as _,
        }];
        vk.device.cmd_copy_buffer(
            command_buffer.buffer,
            staging.buffer.0,
            out.buffer.0,
            &regions,
        );

        out
    }
}

impl<'a> CommittedImage<'a> {
    pub unsafe fn create(
        vk: &'a VkContext,
        format: vk::Format,
        extent: vk::Extent2D,
        mip_levels: u32,
        samples: vk::SampleCountFlags,
        usage: vk::ImageUsageFlags,
        aspect_mask: vk::ImageAspectFlags,
    ) -> Self {
        let queue_family_indices = [vk.physical_device.queue_family_index_graphics];
        let create_info = vk::ImageCreateInfo::default()
            .image_type(vk::ImageType::TYPE_2D)
            .format(format)
            .extent(extent.into())
            .mip_levels(mip_levels)
            .array_layers(1)
            .samples(samples)
            .tiling(vk::ImageTiling::OPTIMAL)
            .usage(usage)
            .sharing_mode(vk::SharingMode::EXCLUSIVE)
            .queue_family_indices(&queue_family_indices)
            .initial_layout(vk::ImageLayout::UNDEFINED);
        let image = vkbox::Image::create(vk, &create_info);
        let memory_requirements = vk.device.get_image_memory_requirements(image.0);
        let memory = vk.allocate_memory(memory_requirements, vk::MemoryPropertyFlags::DEVICE_LOCAL);
        vk.device.bind_image_memory(image.0, memory.0, 0).unwrap();
        let view = vk.create_image_view(image.0, format, aspect_mask, mip_levels);
        Self {
            image,
            view,
            _memory: memory,
        }
    }

    pub unsafe fn upload(
        vk: &'a VkContext,
        command_pool: vk::CommandPool,
        extent: vk::Extent2D,
        srgb: &[u8],
    ) -> Self {
        assert_eq!(srgb.len() as u32, 4 * extent.width * extent.height);
        let mip_levels = extent.width.max(extent.height).ilog2() + 1;
        let staging = CommittedBuffer::new_staging(vk, srgb);
        let out = Self::create(
            vk,
            vk::Format::R8G8B8A8_UNORM,
            extent,
            mip_levels,
            vk::SampleCountFlags::TYPE_1,
            vk::ImageUsageFlags::TRANSFER_SRC
                | vk::ImageUsageFlags::TRANSFER_DST
                | vk::ImageUsageFlags::SAMPLED,
            vk::ImageAspectFlags::COLOR,
        );

        let command_buffer = TransientGraphicsCommandBuffer::begin(vk, command_pool);

        let image_memory_barriers = [vk::ImageMemoryBarrier::default()
            .src_access_mask(vk::AccessFlags::empty())
            .dst_access_mask(vk::AccessFlags::TRANSFER_WRITE)
            .old_layout(vk::ImageLayout::UNDEFINED)
            .new_layout(vk::ImageLayout::TRANSFER_DST_OPTIMAL)
            .src_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
            .dst_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
            .image(out.image.0)
            .subresource_range(vk::ImageSubresourceRange {
                aspect_mask: vk::ImageAspectFlags::COLOR,
                base_mip_level: 0,
                level_count: mip_levels,
                base_array_layer: 0,
                layer_count: 1,
            })];
        vk.device.cmd_pipeline_barrier(
            command_buffer.buffer,
            vk::PipelineStageFlags::TOP_OF_PIPE,
            vk::PipelineStageFlags::TRANSFER,
            vk::DependencyFlags::empty(),
            &[],
            &[],
            &image_memory_barriers,
        );

        let regions = [vk::BufferImageCopy {
            buffer_offset: 0,
            buffer_row_length: 0,
            buffer_image_height: 0,
            image_subresource: vk::ImageSubresourceLayers {
                aspect_mask: vk::ImageAspectFlags::COLOR,
                mip_level: 0,
                base_array_layer: 0,
                layer_count: 1,
            },
            image_offset: vk::Offset3D { x: 0, y: 0, z: 0 },
            image_extent: extent.into(),
        }];
        vk.device.cmd_copy_buffer_to_image(
            command_buffer.buffer,
            staging.buffer.0,
            out.image.0,
            vk::ImageLayout::TRANSFER_DST_OPTIMAL,
            &regions,
        );

        let mut mip_offset = vk::Offset3D {
            x: extent.width as _,
            y: extent.height as _,
            z: 1,
        };
        for i in 1..mip_levels {
            let image_memory_barriers = [vk::ImageMemoryBarrier::default()
                .src_access_mask(vk::AccessFlags::TRANSFER_WRITE)
                .dst_access_mask(vk::AccessFlags::TRANSFER_READ)
                .old_layout(vk::ImageLayout::TRANSFER_DST_OPTIMAL)
                .new_layout(vk::ImageLayout::TRANSFER_SRC_OPTIMAL)
                .src_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
                .dst_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
                .image(out.image.0)
                .subresource_range(vk::ImageSubresourceRange {
                    aspect_mask: vk::ImageAspectFlags::COLOR,
                    base_mip_level: i - 1,
                    level_count: 1,
                    base_array_layer: 0,
                    layer_count: 1,
                })];
            vk.device.cmd_pipeline_barrier(
                command_buffer.buffer,
                vk::PipelineStageFlags::TRANSFER,
                vk::PipelineStageFlags::TRANSFER,
                vk::DependencyFlags::empty(),
                &[],
                &[],
                &image_memory_barriers,
            );

            let next_mip_offset = vk::Offset3D {
                x: 1.max(mip_offset.x / 2),
                y: 1.max(mip_offset.y / 2),
                z: 1,
            };
            let regions = [vk::ImageBlit {
                src_subresource: vk::ImageSubresourceLayers {
                    aspect_mask: vk::ImageAspectFlags::COLOR,
                    mip_level: i - 1,
                    base_array_layer: 0,
                    layer_count: 1,
                },
                src_offsets: [vk::Offset3D { x: 0, y: 0, z: 0 }, mip_offset],
                dst_subresource: vk::ImageSubresourceLayers {
                    aspect_mask: vk::ImageAspectFlags::COLOR,
                    mip_level: i,
                    base_array_layer: 0,
                    layer_count: 1,
                },
                dst_offsets: [vk::Offset3D { x: 0, y: 0, z: 0 }, next_mip_offset],
            }];
            mip_offset = next_mip_offset;

            vk.device.cmd_blit_image(
                command_buffer.buffer,
                out.image.0,
                vk::ImageLayout::TRANSFER_SRC_OPTIMAL,
                out.image.0,
                vk::ImageLayout::TRANSFER_DST_OPTIMAL,
                &regions,
                vk::Filter::LINEAR,
            );
        }

        let image_memory_barriers = [
            vk::ImageMemoryBarrier::default()
                .src_access_mask(vk::AccessFlags::TRANSFER_READ)
                .dst_access_mask(vk::AccessFlags::SHADER_READ)
                .old_layout(vk::ImageLayout::TRANSFER_SRC_OPTIMAL)
                .new_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL)
                .src_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
                .dst_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
                .image(out.image.0)
                .subresource_range(vk::ImageSubresourceRange {
                    aspect_mask: vk::ImageAspectFlags::COLOR,
                    base_mip_level: 0,
                    level_count: mip_levels - 1,
                    base_array_layer: 0,
                    layer_count: 1,
                }),
            vk::ImageMemoryBarrier::default()
                .src_access_mask(vk::AccessFlags::TRANSFER_WRITE)
                .dst_access_mask(vk::AccessFlags::SHADER_READ)
                .old_layout(vk::ImageLayout::TRANSFER_DST_OPTIMAL)
                .new_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL)
                .src_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
                .dst_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
                .image(out.image.0)
                .subresource_range(vk::ImageSubresourceRange {
                    aspect_mask: vk::ImageAspectFlags::COLOR,
                    base_mip_level: mip_levels - 1,
                    level_count: 1,
                    base_array_layer: 0,
                    layer_count: 1,
                }),
        ];
        vk.device.cmd_pipeline_barrier(
            command_buffer.buffer,
            vk::PipelineStageFlags::TRANSFER,
            vk::PipelineStageFlags::FRAGMENT_SHADER,
            vk::DependencyFlags::empty(),
            &[],
            &[],
            &image_memory_barriers,
        );
        out
    }
}
