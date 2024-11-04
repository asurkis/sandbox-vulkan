mod math;
mod state;
mod vklib;

use {
    ash::vk::{self, BufferUsageFlags},
    image::EncodableLayout,
    math::{mat4, vec2, vec3, Vector},
    sdl2::event::Event,
    state::StateBox,
    std::{ffi::CStr, mem, ptr, slice, time, u64},
    vklib::{vkbox, CommittedBuffer, CommittedImage, SdlContext, Swapchain, VkContext},
};

const MAX_CONCURRENT_FRAMES: usize = 2;

const BYTECODE_VERT: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/main.vert.spv"));
const BYTECODE_FRAG: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/main.frag.spv"));

#[derive(Clone, Copy, Debug, Default)]
struct UniformData {
    mat_view: mat4,
    mat_proj: mat4,
    mat_view_proj: mat4,
}

#[derive(Clone, Copy, Debug, Default)]
struct Vertex {
    pos: vec3,
    texcoord: vec2,
}

fn main() {
    unsafe {
        let mut state = StateBox::load("state.json".into());

        let mut sdl = SdlContext::new();
        let vk = VkContext::new(&sdl.window);

        let physical_device_props = vk
            .instance
            .get_physical_device_properties(vk.physical_device.physical_device);
        let mut msaa_sample_count = vk::SampleCountFlags::TYPE_1;
        for candidate in [
            vk::SampleCountFlags::TYPE_64,
            vk::SampleCountFlags::TYPE_32,
            vk::SampleCountFlags::TYPE_16,
            vk::SampleCountFlags::TYPE_8,
            vk::SampleCountFlags::TYPE_4,
            vk::SampleCountFlags::TYPE_2,
            vk::SampleCountFlags::TYPE_1,
        ] {
            if candidate
                & physical_device_props.limits.framebuffer_color_sample_counts
                & physical_device_props.limits.framebuffer_depth_sample_counts
                == candidate
            {
                msaa_sample_count = candidate;
                break;
            }
        }

        let depth_buffer_format = vk.select_image_format(
            &[
                vk::Format::D32_SFLOAT,
                vk::Format::D32_SFLOAT_S8_UINT,
                vk::Format::D24_UNORM_S8_UINT,
            ],
            vk::ImageTiling::OPTIMAL,
            vk::FormatFeatureFlags::DEPTH_STENCIL_ATTACHMENT,
        );
        let command_pool = vk.create_graphics_command_pool();
        let command_pool_transient = vk.create_graphics_transient_command_pool();
        let render_pass = create_render_pass(&vk, depth_buffer_format, msaa_sample_count);
        let mut swapchain = Swapchain::new(
            &vk,
            command_pool_transient.0,
            render_pass.0,
            depth_buffer_format,
            msaa_sample_count,
            None,
        );
        let pipeline = PipelineBox::new(&vk, render_pass.0, msaa_sample_count);

        let mut imgui = imgui::Context::create();
        // imgui.set_ini_filename(None);
        let mut imgui_sdl = imgui_sdl2_support::SdlPlatform::new(&mut imgui);
        let mut imgui_renderer = imgui_rs_vulkan_renderer::Renderer::with_default_allocator(
            &vk.instance,
            vk.physical_device.physical_device,
            vk.device.clone(),
            vk.queue_graphics,
            command_pool.0,
            render_pass.0,
            &mut imgui,
            Some(imgui_rs_vulkan_renderer::Options {
                in_flight_frames: MAX_CONCURRENT_FRAMES,
                enable_depth_test: false,
                enable_depth_write: false,
                subpass: 0,
                sample_count: msaa_sample_count,
            }),
        )
        .unwrap();

        let (meshes, _) = tobj::load_obj(
            "assets/viking_room.obj",
            &tobj::LoadOptions {
                ignore_lines: true,
                single_index: true,
                triangulate: true,
                ignore_points: true,
            },
        )
        .unwrap();
        let mesh = &meshes[0].mesh;
        let n_indices = mesh.indices.len();
        let n_vertices = mesh.positions.len() / 3;
        let mut vertex_buffer_data = Vec::with_capacity(n_vertices);
        for i in 0..n_vertices {
            vertex_buffer_data.push(Vertex {
                pos: Vector([
                    mesh.positions[3 * i + 0],
                    mesh.positions[3 * i + 2],
                    mesh.positions[3 * i + 1],
                ]),
                texcoord: Vector([mesh.texcoords[2 * i + 0], 1.0 - mesh.texcoords[2 * i + 1]]),
            });
        }
        let index_buffer = CommittedBuffer::upload(
            &vk,
            command_pool_transient.0,
            &mesh.indices,
            BufferUsageFlags::INDEX_BUFFER,
        );
        let vertex_buffer = CommittedBuffer::upload(
            &vk,
            command_pool_transient.0,
            &vertex_buffer_data,
            BufferUsageFlags::VERTEX_BUFFER,
        );
        let _ = meshes;
        let _ = vertex_buffer_data;

        let texture = {
            let texture_data = image::ImageReader::open("assets/viking_room.png")
                .unwrap()
                .decode()
                .unwrap()
                .to_rgba8();
            CommittedImage::upload(
                &vk,
                command_pool_transient.0,
                vk::Extent2D {
                    width: texture_data.width(),
                    height: texture_data.height(),
                },
                &texture_data.as_bytes(),
            )
        };
        let texture_sampler = vk.create_sampler();

        let mut uniform_data = UniformData::default();
        let uniform_data_size = mem::size_of_val(&uniform_data);

        let command_buffers =
            vk.allocate_command_buffers(command_pool.0, MAX_CONCURRENT_FRAMES as _);
        let mut uniform_buffers = Vec::with_capacity(MAX_CONCURRENT_FRAMES);
        let mut uniform_mappings = Vec::with_capacity(MAX_CONCURRENT_FRAMES);
        let mut semaphores_image_available = Vec::with_capacity(MAX_CONCURRENT_FRAMES);
        let mut semaphores_render_finished = Vec::with_capacity(MAX_CONCURRENT_FRAMES);
        let mut fences_in_flight = Vec::with_capacity(MAX_CONCURRENT_FRAMES);
        for _ in 0..MAX_CONCURRENT_FRAMES {
            let uniform_buffer = CommittedBuffer::new(
                &vk,
                mem::size_of_val(&uniform_data) as _,
                vk::BufferUsageFlags::UNIFORM_BUFFER,
                vk::MemoryPropertyFlags::HOST_VISIBLE | vk::MemoryPropertyFlags::HOST_COHERENT,
            );
            let memory_mapping = vk
                .device
                .map_memory(
                    uniform_buffer.memory.0,
                    0,
                    uniform_data_size as _,
                    vk::MemoryMapFlags::empty(),
                )
                .unwrap();
            uniform_mappings.push(memory_mapping);
            uniform_buffers.push(uniform_buffer);
            semaphores_image_available.push(vk.create_semaphore());
            semaphores_render_finished.push(vk.create_semaphore());
            fences_in_flight.push(vk.create_fence_signaled());
        }

        let descriptor_pool = create_descriptor_pool(&vk);
        let descriptor_sets = create_descriptor_sets(
            &vk,
            descriptor_pool.0,
            pipeline.descriptor_set_layout.0,
            texture_sampler.0,
            texture.view.0,
            &uniform_buffers,
        );

        let mut frame_in_flight_index = 0;

        let mut time_prev = time::Instant::now();

        'main_loop: loop {
            imgui_sdl.prepare_frame(&mut imgui, &sdl.window, &sdl.event_pump);
            for evt in sdl.event_pump.poll_iter() {
                if !imgui_sdl.handle_event(&mut imgui, &evt) {
                    continue;
                }
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
                        swapchain.reinit();
                        continue 'main_loop;
                    }
                    _ => {}
                }
            }

            let time_curr = time::Instant::now();

            let ui = imgui.new_frame();
            ui.window("Info").build(|| {
                ui.text(format!("FPS: {}", ui.io().framerate));
                ui.slider("Turn speed", -360.0, 360.0, &mut state.turn_speed);
                ui.slider("Angle", -180.0, 180.0, &mut state.angle_deg);
            });

            let time_elapsed = time_curr - time_prev;
            let nanos = time_elapsed.as_secs() * 1_000_000_000 + time_elapsed.subsec_nanos() as u64;
            state.update(nanos);
            time_prev = time_curr;

            let (sin, cos) = state.angle_deg.to_radians().sin_cos();

            let cam_pos = Vector([sin, 1.0, cos]);
            let look_at = Vector([0.0; 3]);
            let world_up = Vector([0.0, 1.0, 0.0]);
            let cam_forward = (look_at - cam_pos).normalize();
            let cam_right = cam_forward.cross(world_up).normalize();
            let cam_down = cam_forward.cross(cam_right);

            let mut mat_transform = mat4::identity();
            mat_transform.0[3][0] = -cam_pos.x();
            mat_transform.0[3][1] = -cam_pos.y();
            mat_transform.0[3][2] = -cam_pos.z();

            uniform_data.mat_view.0[0][0] = cam_right.x();
            uniform_data.mat_view.0[1][0] = cam_right.y();
            uniform_data.mat_view.0[2][0] = cam_right.z();
            uniform_data.mat_view.0[3][0] = 0.0;
            uniform_data.mat_view.0[0][1] = cam_down.x();
            uniform_data.mat_view.0[1][1] = cam_down.y();
            uniform_data.mat_view.0[2][1] = cam_down.z();
            uniform_data.mat_view.0[3][1] = 0.0;
            uniform_data.mat_view.0[0][2] = cam_forward.x();
            uniform_data.mat_view.0[1][2] = cam_forward.y();
            uniform_data.mat_view.0[2][2] = cam_forward.z();
            uniform_data.mat_view.0[3][2] = 0.0;
            uniform_data.mat_view.0[0][3] = 0.0;
            uniform_data.mat_view.0[1][3] = 0.0;
            uniform_data.mat_view.0[2][3] = 0.0;
            uniform_data.mat_view.0[3][3] = 1.0;
            uniform_data.mat_view.dot_assign(&mat_transform);

            uniform_data.mat_proj = mat4::identity();
            uniform_data.mat_proj.0[0][0] =
                swapchain.extent.height as f32 / swapchain.extent.width as f32;
            uniform_data.mat_proj.0[2][3] = 1.0;
            uniform_data.mat_view_proj = uniform_data.mat_proj.dot(&uniform_data.mat_view);

            let cur_command_buffer = command_buffers[frame_in_flight_index];
            let cur_uniform_mapping = uniform_mappings[frame_in_flight_index];
            let cur_fence = fences_in_flight[frame_in_flight_index].0;
            let cur_image_available = semaphores_image_available[frame_in_flight_index].0;
            let cur_render_finished = semaphores_render_finished[frame_in_flight_index].0;
            let cur_descriptor_set = descriptor_sets[frame_in_flight_index];

            ptr::copy(
                mem::transmute(&uniform_data as *const _),
                cur_uniform_mapping,
                uniform_data_size,
            );

            vk.device
                .wait_for_fences(slice::from_ref(&cur_fence), true, u64::MAX)
                .unwrap();
            let result = vk.device_ext_swapchain.acquire_next_image(
                swapchain.swapchain.0,
                u64::MAX,
                cur_image_available,
                vk::Fence::null(),
            );
            let image_index = match result {
                Ok((image_index, false)) => image_index,
                Ok((_, true)) | Err(vk::Result::ERROR_OUT_OF_DATE_KHR) => {
                    swapchain.reinit();
                    continue 'main_loop;
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
            let clear_values = [
                vk::ClearValue {
                    color: vk::ClearColorValue {
                        float32: [1.0, 0.75, 0.5, 0.0],
                    },
                },
                vk::ClearValue {
                    depth_stencil: vk::ClearDepthStencilValue {
                        depth: 1.0,
                        stencil: 0,
                    },
                },
            ];
            let render_pass_begin = vk::RenderPassBeginInfo::default()
                .render_pass(render_pass.0)
                .framebuffer(swapchain.framebuffers[image_index as usize].0)
                .render_area(swapchain.extent.into())
                .clear_values(&clear_values);
            let viewports = [vk::Viewport {
                x: 0.0,
                y: 0.0,
                width: swapchain.extent.width as _,
                height: swapchain.extent.height as _,
                min_depth: 0.0,
                max_depth: 1.0,
            }];
            let scissors = [swapchain.extent.into()];
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
            vk.device.cmd_bind_vertex_buffers(
                cur_command_buffer,
                0,
                &[vertex_buffer.buffer.0],
                &[0],
            );
            vk.device.cmd_bind_index_buffer(
                cur_command_buffer,
                index_buffer.buffer.0,
                0,
                vk::IndexType::UINT32,
            );
            vk.device.cmd_bind_descriptor_sets(
                cur_command_buffer,
                vk::PipelineBindPoint::GRAPHICS,
                pipeline.layout.0,
                0,
                slice::from_ref(&cur_descriptor_set),
                &[],
            );
            vk.device
                .cmd_draw_indexed(cur_command_buffer, n_indices as _, 1, 0, 0, 0);

            imgui_renderer
                .cmd_draw(cur_command_buffer, imgui.render())
                .unwrap();

            vk.device.cmd_end_render_pass(cur_command_buffer);
            vk.device.end_command_buffer(cur_command_buffer).unwrap();

            let wait_stages = [vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT];
            let submit_info = [vk::SubmitInfo::default()
                .wait_semaphores(slice::from_ref(&cur_image_available))
                .wait_dst_stage_mask(&wait_stages)
                .command_buffers(slice::from_ref(&cur_command_buffer))
                .signal_semaphores(slice::from_ref(&cur_render_finished))];
            vk.device.reset_fences(slice::from_ref(&cur_fence)).unwrap();
            vk.device
                .queue_submit(vk.queue_graphics, &submit_info, cur_fence)
                .unwrap();

            let present_info = vk::PresentInfoKHR::default()
                .wait_semaphores(slice::from_ref(&cur_render_finished))
                .swapchains(slice::from_ref(&swapchain.swapchain.0))
                .image_indices(slice::from_ref(&image_index));
            match vk
                .device_ext_swapchain
                .queue_present(vk.queue_present, &present_info)
            {
                Ok(false) => {}
                Ok(true) | Err(vk::Result::ERROR_OUT_OF_DATE_KHR) => swapchain.reinit(),
                Err(err) => panic!("Unexpected Vulkan error: {err}"),
            };

            frame_in_flight_index = (frame_in_flight_index + 1) % MAX_CONCURRENT_FRAMES;
        }

        vk.device.device_wait_idle().unwrap();
    }
}

unsafe fn create_render_pass(
    vk: &VkContext,
    depth_buffer_format: vk::Format,
    samples: vk::SampleCountFlags,
) -> vkbox::RenderPass {
    let msaa_on = samples != vk::SampleCountFlags::TYPE_1;
    let display_format = vk.physical_device.surface_formats[0].format;
    let attachments = {
        let mut base = vec![
            vk::AttachmentDescription {
                flags: vk::AttachmentDescriptionFlags::empty(),
                format: display_format,
                samples,
                load_op: vk::AttachmentLoadOp::CLEAR,
                store_op: vk::AttachmentStoreOp::STORE,
                stencil_load_op: vk::AttachmentLoadOp::DONT_CARE,
                stencil_store_op: vk::AttachmentStoreOp::DONT_CARE,
                initial_layout: vk::ImageLayout::UNDEFINED,
                final_layout: vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
            },
            vk::AttachmentDescription {
                flags: vk::AttachmentDescriptionFlags::empty(),
                format: depth_buffer_format,
                samples,
                load_op: vk::AttachmentLoadOp::CLEAR,
                store_op: vk::AttachmentStoreOp::DONT_CARE,
                stencil_load_op: vk::AttachmentLoadOp::CLEAR,
                stencil_store_op: vk::AttachmentStoreOp::DONT_CARE,
                initial_layout: vk::ImageLayout::UNDEFINED,
                final_layout: vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL,
            },
            vk::AttachmentDescription {
                flags: vk::AttachmentDescriptionFlags::empty(),
                format: display_format,
                samples: vk::SampleCountFlags::TYPE_1,
                load_op: vk::AttachmentLoadOp::DONT_CARE,
                store_op: vk::AttachmentStoreOp::STORE,
                stencil_load_op: vk::AttachmentLoadOp::DONT_CARE,
                stencil_store_op: vk::AttachmentStoreOp::DONT_CARE,
                initial_layout: vk::ImageLayout::UNDEFINED,
                final_layout: vk::ImageLayout::PRESENT_SRC_KHR,
            },
        ];
        if !msaa_on {
            base.pop();
            base[0].final_layout = vk::ImageLayout::PRESENT_SRC_KHR;
        }
        base
    };
    let color_attachments = [vk::AttachmentReference {
        attachment: 0,
        layout: vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
    }];
    let resolve_attachments = [vk::AttachmentReference {
        attachment: 2,
        layout: vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
    }];
    let depth_stencil_attachment = vk::AttachmentReference {
        attachment: 1,
        layout: vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL,
    };
    let subpasses = [{
        let mut base = vk::SubpassDescription::default()
            .pipeline_bind_point(vk::PipelineBindPoint::GRAPHICS)
            .color_attachments(&color_attachments)
            .depth_stencil_attachment(&depth_stencil_attachment);
        if msaa_on {
            base = base.resolve_attachments(&resolve_attachments)
        }
        base
    }];
    let dependencies = [vk::SubpassDependency {
        src_subpass: vk::SUBPASS_EXTERNAL,
        dst_subpass: 0,
        src_stage_mask: vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT
            | vk::PipelineStageFlags::LATE_FRAGMENT_TESTS,
        dst_stage_mask: vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT
            | vk::PipelineStageFlags::EARLY_FRAGMENT_TESTS,
        src_access_mask: vk::AccessFlags::COLOR_ATTACHMENT_WRITE
            | vk::AccessFlags::DEPTH_STENCIL_ATTACHMENT_WRITE,
        dst_access_mask: vk::AccessFlags::COLOR_ATTACHMENT_WRITE
            | vk::AccessFlags::DEPTH_STENCIL_ATTACHMENT_WRITE,
        dependency_flags: vk::DependencyFlags::empty(),
    }];
    let create_info = vk::RenderPassCreateInfo::default()
        .attachments(&attachments)
        .subpasses(&subpasses)
        .dependencies(&dependencies);
    vkbox::RenderPass::new(vk, &create_info)
}

struct PipelineBox<'a> {
    pipeline: vk::Pipeline,
    layout: vkbox::PipelineLayout<'a>,
    descriptor_set_layout: vkbox::DescriptorSetLayout<'a>,
    vk: &'a VkContext,
}

impl<'a> Drop for PipelineBox<'a> {
    fn drop(&mut self) {
        unsafe {
            self.vk.device.destroy_pipeline(self.pipeline, None);
        }
    }
}

impl<'a> PipelineBox<'a> {
    unsafe fn new(
        vk: &'a VkContext,
        render_pass: vk::RenderPass,
        rasterization_samples: vk::SampleCountFlags,
    ) -> Self {
        let shader_module_vert = vk.create_shader_module(BYTECODE_VERT);
        let shader_module_frag = vk.create_shader_module(BYTECODE_FRAG);

        let stage_create_infos = [
            vk::PipelineShaderStageCreateInfo::default()
                .stage(vk::ShaderStageFlags::VERTEX)
                .module(shader_module_vert.0)
                .name(CStr::from_bytes_with_nul(b"main\0").unwrap()),
            vk::PipelineShaderStageCreateInfo::default()
                .stage(vk::ShaderStageFlags::FRAGMENT)
                .module(shader_module_frag.0)
                .name(CStr::from_bytes_with_nul(b"main\0").unwrap()),
        ];
        let vertex_binding_descriptions = [vk::VertexInputBindingDescription {
            binding: 0,
            stride: mem::size_of::<Vertex>() as _,
            input_rate: vk::VertexInputRate::VERTEX,
        }];
        let vertex_attribute_descriptions = [
            vk::VertexInputAttributeDescription {
                location: 0,
                binding: 0,
                format: vk::Format::R32G32B32_SFLOAT,
                offset: mem::offset_of!(Vertex, pos) as _,
            },
            vk::VertexInputAttributeDescription {
                location: 1,
                binding: 0,
                format: vk::Format::R32G32_SFLOAT,
                offset: mem::offset_of!(Vertex, texcoord) as _,
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
            .front_face(vk::FrontFace::CLOCKWISE)
            .line_width(1.0);
        let multisample_state = vk::PipelineMultisampleStateCreateInfo::default()
            .rasterization_samples(rasterization_samples);
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
        let depth_stencil_state = vk::PipelineDepthStencilStateCreateInfo::default()
            .flags(vk::PipelineDepthStencilStateCreateFlags::empty())
            .depth_test_enable(true)
            .depth_write_enable(true)
            .depth_compare_op(vk::CompareOp::LESS)
            .depth_bounds_test_enable(false)
            .stencil_test_enable(false);
        let color_blend_state =
            vk::PipelineColorBlendStateCreateInfo::default().attachments(&color_blend_attachments);
        let dynamic_states = [vk::DynamicState::VIEWPORT, vk::DynamicState::SCISSOR];
        let dynamic_state_create_info =
            vk::PipelineDynamicStateCreateInfo::default().dynamic_states(&dynamic_states);

        let bindings = [
            vk::DescriptorSetLayoutBinding::default()
                .binding(0)
                .descriptor_type(vk::DescriptorType::UNIFORM_BUFFER)
                .descriptor_count(1)
                .stage_flags(vk::ShaderStageFlags::VERTEX),
            vk::DescriptorSetLayoutBinding::default()
                .binding(1)
                .descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER)
                .descriptor_count(1)
                .stage_flags(vk::ShaderStageFlags::FRAGMENT),
        ];
        let descriptor_set_layout_create_info =
            vk::DescriptorSetLayoutCreateInfo::default().bindings(&bindings);
        let descriptor_set_layout =
            vkbox::DescriptorSetLayout::new(vk, &descriptor_set_layout_create_info);

        let layout_create_info = vk::PipelineLayoutCreateInfo::default()
            .set_layouts(slice::from_ref(&descriptor_set_layout.0));
        let layout = vkbox::PipelineLayout::new(vk, &layout_create_info);
        let pipeline_create_infos = [vk::GraphicsPipelineCreateInfo::default()
            .stages(&stage_create_infos)
            .vertex_input_state(&vertex_input_state)
            .input_assembly_state(&input_assembly_state)
            .viewport_state(&viewport_state)
            .rasterization_state(&rasterization_state)
            .multisample_state(&multisample_state)
            .depth_stencil_state(&depth_stencil_state)
            .color_blend_state(&color_blend_state)
            .dynamic_state(&dynamic_state_create_info)
            .layout(layout.0)
            .render_pass(render_pass)];
        let pipelines = vk
            .device
            .create_graphics_pipelines(vk::PipelineCache::null(), &pipeline_create_infos, None)
            .unwrap();

        Self {
            pipeline: pipelines[0],
            layout,
            descriptor_set_layout,
            vk,
        }
    }
}

unsafe fn create_descriptor_pool(vk: &VkContext) -> vkbox::DescriptorPool {
    let pool_sizes = [
        vk::DescriptorPoolSize {
            ty: vk::DescriptorType::UNIFORM_BUFFER,
            descriptor_count: MAX_CONCURRENT_FRAMES as _,
        },
        vk::DescriptorPoolSize {
            ty: vk::DescriptorType::COMBINED_IMAGE_SAMPLER,
            descriptor_count: MAX_CONCURRENT_FRAMES as _,
        },
    ];
    let create_info = vk::DescriptorPoolCreateInfo::default()
        .flags(vk::DescriptorPoolCreateFlags::empty())
        .max_sets(MAX_CONCURRENT_FRAMES as _)
        .pool_sizes(&pool_sizes);
    vkbox::DescriptorPool::new(vk, &create_info)
}

unsafe fn create_descriptor_sets(
    vk: &VkContext,
    descriptor_pool: vk::DescriptorPool,
    layout: vk::DescriptorSetLayout,
    texture_sampler: vk::Sampler,
    texture_view: vk::ImageView,
    uniform_buffers: &[CommittedBuffer],
) -> Vec<vk::DescriptorSet> {
    let set_layouts = [layout; MAX_CONCURRENT_FRAMES];
    let allocate_info = vk::DescriptorSetAllocateInfo::default()
        .descriptor_pool(descriptor_pool)
        .set_layouts(&set_layouts);
    let sets = vk.device.allocate_descriptor_sets(&allocate_info).unwrap();
    for i in 0..MAX_CONCURRENT_FRAMES {
        let buffer_info = [vk::DescriptorBufferInfo {
            buffer: uniform_buffers[i].buffer.0,
            offset: 0,
            range: mem::size_of::<UniformData>() as _,
        }];
        let image_info = [vk::DescriptorImageInfo {
            sampler: texture_sampler,
            image_view: texture_view,
            image_layout: vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
        }];
        let descriptor_writes = [
            vk::WriteDescriptorSet::default()
                .dst_set(sets[i])
                .dst_binding(0)
                .dst_array_element(0)
                .descriptor_type(vk::DescriptorType::UNIFORM_BUFFER)
                .buffer_info(&buffer_info),
            vk::WriteDescriptorSet::default()
                .dst_set(sets[i])
                .dst_binding(1)
                .dst_array_element(0)
                .descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER)
                .image_info(&image_info),
        ];
        vk.device.update_descriptor_sets(&descriptor_writes, &[]);
    }
    sets
}
