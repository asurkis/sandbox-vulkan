mod bootstrap;
mod holders;

use ash::vk;
use bootstrap::BufferBox;
use bootstrap::SdlBox;
use bootstrap::SwapchainBox;
use bootstrap::VkBox;
use sdl2::event::Event;
use std::ffi::CStr;
use std::slice;
use std::u64;

const MAX_CONCURRENT_FRAMES: usize = 2;

const BYTECODE_VERT: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/main.vert.spv"));
const BYTECODE_FRAG: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/main.frag.spv"));
const VERTEX_DATA: &[([f32; 2], [f32; 3])] = &[
    // position, color
    ([0.5f32, -0.5], [1.0f32, 0.0, 0.0]),
    ([-0.5, -0.5], [0.0, 1.0, 0.0]),
    ([-0.5, 0.5], [0.0, 1.0, 1.0]),
    ([0.5, 0.5], [0.0, 0.0, 1.0]),
];
const INDEX_DATA: &[u16] = &[0, 1, 2, 0, 2, 3];

fn main() {
    unsafe {
        let mut sdl = SdlBox::new();
        let vk = VkBox::new(&sdl);

        let mut swapchain_create_info = vk.physical_device.swapchain_create_info();
        let render_pass = create_render_pass(&vk.device, swapchain_create_info.image_format);

        let mut swapchain = SwapchainBox::new(&vk, swapchain_create_info, render_pass, None);
        let pipeline = PipelineBox::new(&vk, render_pass);
        let command_pool = vk.create_graphics_command_pool();
        let command_pool_transient = vk.create_graphics_transient_command_pool();
        let (vertex_buffer, index_buffer) = create_mesh(&vk, command_pool_transient);

        let command_buffers = vk.allocate_command_buffers(command_pool, MAX_CONCURRENT_FRAMES as _);
        let mut semaphores_image_available = [vk::Semaphore::null(); MAX_CONCURRENT_FRAMES];
        let mut semaphores_render_finished = [vk::Semaphore::null(); MAX_CONCURRENT_FRAMES];
        let mut fences_in_flight = [vk::Fence::null(); MAX_CONCURRENT_FRAMES];
        for i in 0..MAX_CONCURRENT_FRAMES {
            semaphores_image_available[i] = vk.create_semaphore();
            semaphores_render_finished[i] = vk.create_semaphore();
            fences_in_flight[i] = vk.create_fence_signaled();
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
                    Event::Window {
                        win_event: sdl2::event::WindowEvent::Resized(_, _),
                        ..
                    } => {
                        swapchain.reinit(&vk, &mut swapchain_create_info, render_pass);
                        continue 'main_loop;
                    }
                    _ => {}
                }
            }

            let cur_command_buffer = command_buffers[command_buffer_index];
            let cur_fence = fences_in_flight[command_buffer_index];
            let cur_image_available = semaphores_image_available[command_buffer_index];
            let cur_render_finished = semaphores_render_finished[command_buffer_index];

            vk.device
                .wait_for_fences(slice::from_ref(&cur_fence), true, u64::MAX)
                .unwrap();
            let result = vk.device_ext_swapchain.acquire_next_image(
                swapchain.swapchain,
                u64::MAX,
                cur_image_available,
                vk::Fence::null(),
            );
            let image_index = match result {
                Ok((image_index, false)) => image_index,
                Ok((_, true)) | Err(vk::Result::ERROR_OUT_OF_DATE_KHR) => {
                    swapchain.reinit(&vk, &mut swapchain_create_info, render_pass);
                    continue;
                }
                Err(err) => panic!("Unexpected Vulkan error: {err}"),
            };

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
                .framebuffer(swapchain.framebuffers[image_index as usize])
                .render_area(vk::Rect2D {
                    offset: vk::Offset2D { x: 0, y: 0 },
                    extent: swapchain_create_info.image_extent,
                })
                .clear_values(&clear_values);
            let viewports = [vk::Viewport {
                x: 0.0,
                y: 0.0,
                width: swapchain_create_info.image_extent.width as _,
                height: swapchain_create_info.image_extent.height as _,
                min_depth: 0.0,
                max_depth: 1.0,
            }];
            let scissors = [vk::Rect2D {
                offset: vk::Offset2D { x: 0, y: 0 },
                extent: swapchain_create_info.image_extent,
            }];
            vk.device
                .cmd_set_viewport(cur_command_buffer, 0, &viewports);
            vk.device.cmd_set_scissor(cur_command_buffer, 0, &scissors);
            vk.device.cmd_begin_render_pass(
                cur_command_buffer,
                &render_pass_begin,
                vk::SubpassContents::INLINE,
            );
            vk.device.cmd_bind_pipeline(
                cur_command_buffer,
                vk::PipelineBindPoint::GRAPHICS,
                pipeline.pipeline,
            );
            vk.device
                .cmd_bind_vertex_buffers(cur_command_buffer, 0, &[vertex_buffer.buffer], &[0]);
            vk.device.cmd_bind_index_buffer(
                cur_command_buffer,
                index_buffer.buffer,
                0,
                vk::IndexType::UINT16,
            );
            vk.device
                .cmd_draw_indexed(cur_command_buffer, 6, 1, 0, 0, 0);
            vk.device.cmd_end_render_pass(cur_command_buffer);
            vk.device.end_command_buffer(cur_command_buffer).unwrap();

            let wait_stages = [vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT];
            let command_buffers = [cur_command_buffer];
            let submit_info = [vk::SubmitInfo::default()
                .wait_semaphores(slice::from_ref(&cur_image_available))
                .wait_dst_stage_mask(&wait_stages)
                .command_buffers(&command_buffers)
                .signal_semaphores(slice::from_ref(&cur_render_finished))];
            vk.device.reset_fences(slice::from_ref(&cur_fence)).unwrap();
            vk.device
                .queue_submit(vk.queue_graphics, &submit_info, cur_fence)
                .unwrap();

            let swapchains = [swapchain.swapchain];
            let image_indices = [image_index];
            let present_info = vk::PresentInfoKHR::default()
                .wait_semaphores(slice::from_ref(&cur_render_finished))
                .swapchains(&swapchains)
                .image_indices(&image_indices);
            match vk
                .device_ext_swapchain
                .queue_present(vk.queue_present, &present_info)
            {
                Ok(false) => {}
                Ok(true) | Err(vk::Result::ERROR_OUT_OF_DATE_KHR) => {
                    swapchain.reinit(&vk, &mut swapchain_create_info, render_pass);
                }
                Err(err) => panic!("Unexpected Vulkan error: {err}"),
            };

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
        vk.device.destroy_command_pool(command_pool_transient, None);
        vk.device.destroy_command_pool(command_pool, None);
        vk.device.destroy_pipeline(pipeline.pipeline, None);
        vk.device.destroy_pipeline_layout(pipeline.layout, None);
        vertex_buffer.destroy(&vk);
        index_buffer.destroy(&vk);
        swapchain.destroy(&vk);
        vk.device.destroy_render_pass(render_pass, None);
    }
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

#[derive(Debug, Default, Clone, Copy)]
struct PipelineBox {
    pipeline: vk::Pipeline,
    layout: vk::PipelineLayout,
}

impl PipelineBox {
    unsafe fn new(vk: &VkBox, render_pass: vk::RenderPass) -> Self {
        let shader_module_vert = vk.create_shader_module(BYTECODE_VERT);
        let shader_module_frag = vk.create_shader_module(BYTECODE_FRAG);

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
        let vertex_binding_descriptions = [vk::VertexInputBindingDescription {
            binding: 0,
            stride: 20,
            input_rate: vk::VertexInputRate::VERTEX,
        }];
        let vertex_attribute_descriptions = [
            vk::VertexInputAttributeDescription {
                location: 0,
                binding: 0,
                format: vk::Format::R32G32_SFLOAT,
                offset: 0,
            },
            vk::VertexInputAttributeDescription {
                location: 1,
                binding: 0,
                format: vk::Format::R32G32B32_SFLOAT,
                offset: 8,
            },
        ];
        let vertex_input_state = vk::PipelineVertexInputStateCreateInfo::default()
            .vertex_binding_descriptions(&vertex_binding_descriptions)
            .vertex_attribute_descriptions(&vertex_attribute_descriptions);
        let input_assembly_state = vk::PipelineInputAssemblyStateCreateInfo::default()
            .topology(vk::PrimitiveTopology::TRIANGLE_LIST);
        let viewports = [vk::Viewport::default()];
        let scissors = [vk::Rect2D::default()];
        let viewport_state = vk::PipelineViewportStateCreateInfo::default()
            .viewports(&viewports)
            .scissors(&scissors);
        let rasterization_state = vk::PipelineRasterizationStateCreateInfo::default()
            .polygon_mode(vk::PolygonMode::FILL)
            .cull_mode(vk::CullModeFlags::BACK)
            // .front_face(vk::FrontFace::CLOCKWISE)
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
        let dynamic_states = [vk::DynamicState::VIEWPORT, vk::DynamicState::SCISSOR];
        let dynamic_state_create_info =
            vk::PipelineDynamicStateCreateInfo::default().dynamic_states(&dynamic_states);

        let layout_create_info = vk::PipelineLayoutCreateInfo::default();
        let layout = vk
            .device
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
            .dynamic_state(&dynamic_state_create_info)
            .layout(layout)
            .render_pass(render_pass)];
        let pipelines = vk
            .device
            .create_graphics_pipelines(vk::PipelineCache::null(), &pipeline_create_infos, None)
            .unwrap();

        vk.device.destroy_shader_module(shader_module_frag, None);
        vk.device.destroy_shader_module(shader_module_vert, None);

        Self {
            pipeline: pipelines[0],
            layout,
        }
    }
}

unsafe fn create_mesh(vk: &VkBox, command_pool: vk::CommandPool) -> (BufferBox, BufferBox) {
    let vertex_buffer = BufferBox::upload(
        vk,
        &VERTEX_DATA,
        vk::BufferUsageFlags::VERTEX_BUFFER,
        command_pool,
    );
    let index_buffer = BufferBox::upload(
        vk,
        &INDEX_DATA,
        vk::BufferUsageFlags::INDEX_BUFFER,
        command_pool,
    );
    (vertex_buffer, index_buffer)
}
