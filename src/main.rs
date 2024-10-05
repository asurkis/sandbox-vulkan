mod bootstrap;

use ash::vk;
use bootstrap::PhysicalDeviceInfo;
use bootstrap::SdlBox;
use bootstrap::VkBox;
use sdl2::event::Event;
use std::ffi::CStr;
use std::ptr;
use std::u64;

const BYTECODE_VERT: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/main.vert.spv"));
const BYTECODE_FRAG: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/main.frag.spv"));
const MAX_CONCURRENT_FRAMES: usize = 2;

fn main() {
    unsafe {
        let mut sdl = SdlBox::new();
        let vk = VkBox::new(&sdl);

        let swapchain_info = SwapchainInfo::new(&vk.device_ext_swapchain, &vk.physical_device_info);
        let swapchain_image_views = create_image_views(
            &vk.device,
            &swapchain_info.images,
            swapchain_info.image_format,
        );
        let render_pass = create_render_pass(&vk.device, swapchain_info.image_format);
        let pipeline_info = PipelineInfo::new(&vk.device, render_pass);
        let framebuffers = create_framebuffers(&vk.device, &swapchain_image_views, render_pass);
        let [command_pool] = create_command_pool(
            &vk.device,
            [vk.physical_device_info.queue_family_index_graphics],
        );

        let command_buffers = create_command_buffers(&vk.device, command_pool);
        let mut semaphores_image_available = [vk::Semaphore::null(); MAX_CONCURRENT_FRAMES];
        let mut semaphores_render_finished = [vk::Semaphore::null(); MAX_CONCURRENT_FRAMES];
        let mut fences_in_flight = [vk::Fence::null(); MAX_CONCURRENT_FRAMES];
        for i in 0..MAX_CONCURRENT_FRAMES {
            semaphores_image_available[i] = create_semaphore(&vk.device);
            semaphores_render_finished[i] = create_semaphore(&vk.device);
            fences_in_flight[i] = create_fence(&vk.device);
        }
        let mut command_buffer_index = 0;

        'main_loop: loop {
            for evt in sdl.event_pump.poll_iter() {
                match evt {
                    Event::Quit { .. }
                    | Event::KeyDown {
                        keycode: Some(sdl2::keyboard::Keycode::Escape),
                        ..
                    } => break 'main_loop,
                    _ => {}
                }
            }

            let cur_command_buffer = command_buffers[command_buffer_index];
            let cur_fence = [fences_in_flight[command_buffer_index]];
            let cur_image_available = [semaphores_image_available[command_buffer_index]];
            let cur_render_finished = [semaphores_render_finished[command_buffer_index]];

            vk.device
                .wait_for_fences(&cur_fence, true, u64::MAX)
                .unwrap();
            vk.device.reset_fences(&cur_fence).unwrap();
            let (image_index, suboptimal) = vk
                .device_ext_swapchain
                .acquire_next_image(
                    swapchain_info.swapchain,
                    u64::MAX,
                    cur_image_available[0],
                    vk::Fence::null(),
                )
                .unwrap();
            assert!(!suboptimal);

            vk.device
                .reset_command_buffer(cur_command_buffer, vk::CommandBufferResetFlags::empty())
                .unwrap();
            let begin_info = vk::CommandBufferBeginInfo::default();
            vk.device
                .begin_command_buffer(cur_command_buffer, &begin_info)
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
            vk.device.cmd_begin_render_pass(
                cur_command_buffer,
                &render_pass_begin,
                vk::SubpassContents::INLINE,
            );
            vk.device.cmd_bind_pipeline(
                cur_command_buffer,
                vk::PipelineBindPoint::GRAPHICS,
                pipeline_info.pipeline,
            );
            vk.device.cmd_draw(cur_command_buffer, 3, 1, 0, 0);
            vk.device.cmd_end_render_pass(cur_command_buffer);
            vk.device.end_command_buffer(cur_command_buffer).unwrap();

            let wait_stages = [vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT];
            let command_buffers = [cur_command_buffer];
            let submit_info = [vk::SubmitInfo::default()
                .wait_semaphores(&cur_image_available)
                .wait_dst_stage_mask(&wait_stages)
                .command_buffers(&command_buffers)
                .signal_semaphores(&cur_render_finished)];
            vk.device
                .queue_submit(vk.queue_graphics, &submit_info, cur_fence[0])
                .unwrap();

            let swapchains = [swapchain_info.swapchain];
            let image_indices = [image_index];
            let present_info = vk::PresentInfoKHR::default()
                .wait_semaphores(&cur_render_finished)
                .swapchains(&swapchains)
                .image_indices(&image_indices);
            vk.device_ext_swapchain
                .queue_present(vk.queue_present, &present_info)
                .unwrap();

            command_buffer_index = (command_buffer_index + 1) % MAX_CONCURRENT_FRAMES;
        }

        vk.device.device_wait_idle().unwrap();

        for i in 0..MAX_CONCURRENT_FRAMES {
            vk.device.destroy_fence(fences_in_flight[i], None);
            vk.device
                .destroy_semaphore(semaphores_image_available[i], None);
            vk.device
                .destroy_semaphore(semaphores_render_finished[i], None);
        }
        vk.device.destroy_command_pool(command_pool, None);
        for framebuffer in framebuffers {
            vk.device.destroy_framebuffer(framebuffer, None);
        }
        vk.device.destroy_pipeline(pipeline_info.pipeline, None);
        vk.device
            .destroy_pipeline_layout(pipeline_info.layout, None);
        vk.device.destroy_render_pass(render_pass, None);
        for view in swapchain_image_views {
            vk.device.destroy_image_view(view, None);
        }
        vk.device_ext_swapchain
            .destroy_swapchain(swapchain_info.swapchain, None);
    }
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
        pdi: &PhysicalDeviceInfo,
    ) -> Self {
        let queue_family_indices = [
            pdi.queue_family_index_graphics,
            pdi.queue_family_index_present,
        ];
        let mut swapchain_create_info = vk::SwapchainCreateInfoKHR::default()
            .surface(pdi.surface)
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

unsafe fn create_command_buffers(
    device: &ash::Device,
    command_pool: vk::CommandPool,
) -> Vec<vk::CommandBuffer> {
    let allocate_info = vk::CommandBufferAllocateInfo::default()
        .command_pool(command_pool)
        .level(vk::CommandBufferLevel::PRIMARY)
        .command_buffer_count(MAX_CONCURRENT_FRAMES as u32);
    device.allocate_command_buffers(&allocate_info).unwrap()
}

unsafe fn create_semaphore(device: &ash::Device) -> vk::Semaphore {
    let create_info = vk::SemaphoreCreateInfo::default();
    device.create_semaphore(&create_info, None).unwrap()
}

unsafe fn create_fence(device: &ash::Device) -> vk::Fence {
    let create_info = vk::FenceCreateInfo::default().flags(vk::FenceCreateFlags::SIGNALED);
    device.create_fence(&create_info, None).unwrap()
}
