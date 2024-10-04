use ash::vk;
use ash::vk::Handle;
use sdl2::event::Event;
use std::collections::HashSet;
use std::ffi::CStr;
use std::ffi::CString;
use std::ptr;
use std::u64;

const BYTECODE_VERT: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/main.vert.spv"));
const BYTECODE_FRAG: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/main.frag.spv"));

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
        let mut event_pump = sdl_context.event_pump().unwrap();

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
        let render_pass = create_render_pass(&device, swapchain_info.image_format);
        let pipeline_info = PipelineInfo::new(&device, render_pass);
        let framebuffers = create_framebuffers(&device, &swapchain_image_views, render_pass);
        let [command_pool] =
            create_command_pool(&device, [physical_device_info.queue_family_index_graphics]);
        let command_buffer = create_command_buffer(&device, command_pool);

        let semaphore_image_available = create_semaphore(&device);
        let semaphore_render_finished = create_semaphore(&device);
        let fence_command_buffer_in_flight = create_fence(&device);

        'main_loop: loop {
            for evt in event_pump.poll_iter() {
                match evt {
                    Event::Quit { .. }
                    | Event::KeyDown {
                        keycode: Some(sdl2::keyboard::Keycode::Escape),
                        ..
                    } => break 'main_loop,
                    _ => {}
                }
            }

            let fences_to_wait = [fence_command_buffer_in_flight];
            device
                .wait_for_fences(&fences_to_wait, true, u64::MAX)
                .unwrap();
            device.reset_fences(&fences_to_wait).unwrap();
            let (image_index, suboptimal) = device_ext_swapchain
                .acquire_next_image(
                    swapchain_info.swapchain,
                    u64::MAX,
                    semaphore_image_available,
                    vk::Fence::null(),
                )
                .unwrap();
            assert!(!suboptimal);

            device
                .reset_command_buffer(command_buffer, vk::CommandBufferResetFlags::empty())
                .unwrap();
            let begin_info = vk::CommandBufferBeginInfo::default();
            device
                .begin_command_buffer(command_buffer, &begin_info)
                .unwrap();
            let clear_values = [vk::ClearValue::default()];
            let render_pass_begin = vk::RenderPassBeginInfo::default()
                .render_pass(render_pass)
                .framebuffer(framebuffers[image_index as usize])
                .render_area(vk::Rect2D {
                    offset: vk::Offset2D { x: 0, y: 0 },
                    extent: vk::Extent2D {
                        width: 1280,
                        height: 720,
                    },
                })
                .clear_values(&clear_values);
            device.cmd_begin_render_pass(
                command_buffer,
                &render_pass_begin,
                vk::SubpassContents::INLINE,
            );
            device.cmd_bind_pipeline(
                command_buffer,
                vk::PipelineBindPoint::GRAPHICS,
                pipeline_info.pipeline,
            );
            device.cmd_draw(command_buffer, 3, 1, 0, 0);
            device.cmd_end_render_pass(command_buffer);
            device.end_command_buffer(command_buffer).unwrap();

            let wait_semaphores = [semaphore_image_available];
            let wait_stages = [vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT];
            let command_buffers = [command_buffer];
            let signal_semaphores = [semaphore_render_finished];
            let submit_info = [vk::SubmitInfo::default()
                .wait_semaphores(&wait_semaphores)
                .wait_dst_stage_mask(&wait_stages)
                .command_buffers(&command_buffers)
                .signal_semaphores(&signal_semaphores)];
            device
                .queue_submit(queue_graphics, &submit_info, fence_command_buffer_in_flight)
                .unwrap();

            let swapchains = [swapchain_info.swapchain];
            let image_indices = [image_index];
            let present_info = vk::PresentInfoKHR::default()
                .wait_semaphores(&signal_semaphores)
                .swapchains(&swapchains)
                .image_indices(&image_indices);
            device_ext_swapchain
                .queue_present(queue_present, &present_info)
                .unwrap();
        }

        device.device_wait_idle().unwrap();

        device.destroy_fence(fence_command_buffer_in_flight, None);
        device.destroy_semaphore(semaphore_image_available, None);
        device.destroy_semaphore(semaphore_render_finished, None);
        device.destroy_command_pool(command_pool, None);
        for framebuffer in framebuffers {
            device.destroy_framebuffer(framebuffer, None);
        }
        device.destroy_pipeline(pipeline_info.pipeline, None);
        device.destroy_pipeline_layout(pipeline_info.layout, None);
        device.destroy_render_pass(render_pass, None);
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

unsafe fn create_render_pass(device: &ash::Device, format: vk::Format) -> vk::RenderPass {
    let attachments = [vk::AttachmentDescription {
        flags: vk::AttachmentDescriptionFlags::empty(),
        format,
        samples: vk::SampleCountFlags::TYPE_1,
        load_op: vk::AttachmentLoadOp::CLEAR,
        store_op: vk::AttachmentStoreOp::STORE,
        stencil_load_op: vk::AttachmentLoadOp::DONT_CARE,
        stencil_store_op: vk::AttachmentStoreOp::DONT_CARE,
        initial_layout: vk::ImageLayout::UNDEFINED,
        final_layout: vk::ImageLayout::PRESENT_SRC_KHR,
    }];
    let color_attachments = [vk::AttachmentReference {
        attachment: 0,
        layout: vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
    }];
    let subpasses = [vk::SubpassDescription::default()
        .pipeline_bind_point(vk::PipelineBindPoint::GRAPHICS)
        .color_attachments(&color_attachments)];
    let dependencies = [vk::SubpassDependency {
        src_subpass: vk::SUBPASS_EXTERNAL,
        dst_subpass: 0,
        src_stage_mask: vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT,
        dst_stage_mask: vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT,
        src_access_mask: vk::AccessFlags::empty(),
        dst_access_mask: vk::AccessFlags::COLOR_ATTACHMENT_WRITE,
        dependency_flags: vk::DependencyFlags::empty(),
    }];
    let create_info = vk::RenderPassCreateInfo::default()
        .attachments(&attachments)
        .subpasses(&subpasses)
        .dependencies(&dependencies);
    device.create_render_pass(&create_info, None).unwrap()
}

#[derive(Debug, Default, Clone)]
struct PipelineInfo {
    pipeline: vk::Pipeline,
    layout: vk::PipelineLayout,
}

impl PipelineInfo {
    unsafe fn new(device: &ash::Device, render_pass: vk::RenderPass) -> Self {
        let shader_module_vert = create_shader_module(&device, BYTECODE_VERT);
        let shader_module_frag = create_shader_module(&device, BYTECODE_FRAG);

        let stage_create_infos = [
            vk::PipelineShaderStageCreateInfo::default()
                .stage(vk::ShaderStageFlags::VERTEX)
                .module(shader_module_vert)
                .name(CStr::from_bytes_with_nul(b"main\0").unwrap()),
            vk::PipelineShaderStageCreateInfo::default()
                .stage(vk::ShaderStageFlags::FRAGMENT)
                .module(shader_module_frag)
                .name(CStr::from_bytes_with_nul(b"main\0").unwrap()),
        ];
        let vertex_input_state = vk::PipelineVertexInputStateCreateInfo::default();
        let input_assembly_state = vk::PipelineInputAssemblyStateCreateInfo::default()
            .topology(vk::PrimitiveTopology::TRIANGLE_LIST);
        let viewports = [vk::Viewport {
            x: 0.0,
            y: 0.0,
            width: 1280.0,
            height: 720.0,
            min_depth: 0.0,
            max_depth: 1.0,
        }];
        let scissors = [vk::Rect2D {
            offset: vk::Offset2D { x: 0, y: 0 },
            extent: vk::Extent2D {
                width: 1280,
                height: 720,
            },
        }];
        let viewport_state = vk::PipelineViewportStateCreateInfo::default()
            .viewports(&viewports)
            .scissors(&scissors);
        let rasterization_state = vk::PipelineRasterizationStateCreateInfo::default()
            .polygon_mode(vk::PolygonMode::FILL)
            // .cull_mode(vk::CullModeFlags::BACK)
            // .front_face(vk::FrontFace::COUNTER_CLOCKWISE)
            .line_width(1.0);
        let multisample_state = vk::PipelineMultisampleStateCreateInfo::default()
            .rasterization_samples(vk::SampleCountFlags::TYPE_1);
        let color_blend_attachments = [vk::PipelineColorBlendAttachmentState {
            blend_enable: vk::FALSE,
            src_color_blend_factor: vk::BlendFactor::ONE,
            dst_color_blend_factor: vk::BlendFactor::ZERO,
            color_blend_op: vk::BlendOp::ADD,
            src_alpha_blend_factor: vk::BlendFactor::ONE,
            dst_alpha_blend_factor: vk::BlendFactor::ZERO,
            alpha_blend_op: vk::BlendOp::ADD,
            color_write_mask: vk::ColorComponentFlags::A
                | vk::ColorComponentFlags::B
                | vk::ColorComponentFlags::G
                | vk::ColorComponentFlags::R,
        }];
        let color_blend_state =
            vk::PipelineColorBlendStateCreateInfo::default().attachments(&color_blend_attachments);
        // let dynamic_states = [vk::DynamicState::VIEWPORT, vk::DynamicState::SCISSOR];
        // let dynamic_state_create_info =
        //     vk::PipelineDynamicStateCreateInfo::default().dynamic_states(&dynamic_states);

        let layout_create_info = vk::PipelineLayoutCreateInfo::default();
        let layout = device
            .create_pipeline_layout(&layout_create_info, None)
            .unwrap();
        let pipeline_create_infos = [vk::GraphicsPipelineCreateInfo::default()
            .stages(&stage_create_infos)
            .vertex_input_state(&vertex_input_state)
            .input_assembly_state(&input_assembly_state)
            .viewport_state(&viewport_state)
            .rasterization_state(&rasterization_state)
            .multisample_state(&multisample_state)
            .color_blend_state(&color_blend_state)
            // .dynamic_state(&dynamic_state_create_info)
            .layout(layout)
            .render_pass(render_pass)];
        let pipelines = device
            .create_graphics_pipelines(vk::PipelineCache::null(), &pipeline_create_infos, None)
            .unwrap();

        device.destroy_shader_module(shader_module_frag, None);
        device.destroy_shader_module(shader_module_vert, None);

        Self {
            pipeline: pipelines[0],
            layout,
        }
    }
}

unsafe fn create_shader_module(device: &ash::Device, bytecode: &[u8]) -> vk::ShaderModule {
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
    device.create_shader_module(&create_info, None).unwrap()
}

unsafe fn create_framebuffers(
    device: &ash::Device,
    image_views: &[vk::ImageView],
    render_pass: vk::RenderPass,
) -> Vec<vk::Framebuffer> {
    let n = image_views.len();
    let mut framebuffers = Vec::with_capacity(n);
    for &iv in image_views {
        let attachments = [iv];
        let create_info = vk::FramebufferCreateInfo::default()
            .render_pass(render_pass)
            .attachments(&attachments)
            .width(1280)
            .height(720)
            .layers(1);
        let framebuffer = device.create_framebuffer(&create_info, None).unwrap();
        framebuffers.push(framebuffer);
    }
    framebuffers
}

unsafe fn create_command_pool<const N: usize>(
    device: &ash::Device,
    queue_family_indices: [u32; N],
) -> [vk::CommandPool; N] {
    let mut command_pools = [vk::CommandPool::null(); N];
    for i in 0..N {
        let create_info = vk::CommandPoolCreateInfo::default()
            .flags(vk::CommandPoolCreateFlags::RESET_COMMAND_BUFFER)
            .queue_family_index(queue_family_indices[i]);
        command_pools[i] = device.create_command_pool(&create_info, None).unwrap();
    }
    command_pools
}

unsafe fn create_command_buffer(
    device: &ash::Device,
    command_pool: vk::CommandPool,
) -> vk::CommandBuffer {
    let allocate_info = vk::CommandBufferAllocateInfo::default()
        .command_pool(command_pool)
        .level(vk::CommandBufferLevel::PRIMARY)
        .command_buffer_count(1);
    device.allocate_command_buffers(&allocate_info).unwrap()[0]
}

unsafe fn create_semaphore(device: &ash::Device) -> vk::Semaphore {
    let create_info = vk::SemaphoreCreateInfo::default();
    device.create_semaphore(&create_info, None).unwrap()
}

unsafe fn create_fence(device: &ash::Device) -> vk::Fence {
    let create_info = vk::FenceCreateInfo::default().flags(vk::FenceCreateFlags::SIGNALED);
    device.create_fence(&create_info, None).unwrap()
}
