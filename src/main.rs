mod math;
mod state;
mod vkapp;
mod vklib;
mod voxel;

use ash::vk;
use math::{mat4, vec3, vec4, Vector};
use sdl2::event::Event;
use state::StateBox;
use std::{mem, ptr, slice, time};
use vkapp::{
    create_descriptor_pool, create_descriptor_sets_filter, create_descriptor_sets_main,
    create_descriptor_sets_simulation, create_render_pass, update_descriptor_sets_filter,
    PipelineBox, PipelineVec, Swapchain,
};
use vklib::{CommittedBuffer, SdlContext, VkContext};

const MAX_CONCURRENT_FRAMES: usize = 2;
const MAX_PARTICLE_COUNT: usize = 1 << 16;

#[derive(Clone, Copy, Debug, Default)]
struct CameraData {
    mat_view: mat4,
    mat_proj: mat4,
    mat_view_proj: mat4,
}

#[derive(Clone, Copy, Debug, Default)]
struct SimulationStepParams {
    init_pos: vec4, // w --- acceptable deviation
    init_vel: vec4, // w --- acceptable deviation
    acc: vec4,      // w --- unused
    particle_count: u32,
    rng_seed: u32,
    time_step: f32,
    init_ttl: f32,
}

#[derive(Clone, Copy, Debug, Default)]
struct Vertex {
    pos: vec3,
    norm: vec3,
}

#[derive(Clone, Copy, Debug, Default)]
struct Particle {
    pos: vec4,
    vel: vec4,
}

fn main() {
    let mut state = StateBox::load("state.json".into());

    unsafe {
        let mut sdl = SdlContext::new();
        let vk = VkContext::new(&sdl.window);

        let msaa_sample_count = vk.select_msaa_samples();
        // let msaa_sample_count = vk::SampleCountFlags::TYPE_1;
        let hdr_buffer_format = vk.select_image_format(
            &[
                vk::Format::R16G16B16_SFLOAT,
                vk::Format::R16G16B16A16_SFLOAT,
                vk::Format::R32G32B32_SFLOAT,
                vk::Format::R32G32B32A32_SFLOAT,
                vk::Format::R64G64B64_SFLOAT,
                vk::Format::R64G64B64A64_SFLOAT,
            ],
            vk::ImageTiling::OPTIMAL,
            vk::FormatFeatureFlags::SAMPLED_IMAGE
                | vk::FormatFeatureFlags::COLOR_ATTACHMENT
                | vk::FormatFeatureFlags::COLOR_ATTACHMENT_BLEND,
        );
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
        let render_pass = create_render_pass(
            &vk,
            hdr_buffer_format,
            depth_buffer_format,
            msaa_sample_count,
        );
        let mut swapchain = Swapchain::new(
            &vk,
            command_pool_transient.0,
            render_pass.0,
            hdr_buffer_format,
            depth_buffer_format,
            msaa_sample_count,
            None,
        );
        let pipeline_simulate = PipelineBox::new_simulation(&vk);
        // let pipeline_main = PipelineBox::new_main(&vk, render_pass.0, msaa_sample_count);
        let pipeline_particle = PipelineBox::new_particle(&vk, render_pass.0, msaa_sample_count);
        let pipeline_filter = PipelineVec::new_filters(&vk, render_pass.0);

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
                subpass: 4,
                sample_count: vk::SampleCountFlags::TYPE_1,
            }),
        )
        .unwrap();

        let particles_buffer = CommittedBuffer::upload(
            &vk,
            command_pool_transient.0,
            &vec![Particle::default(); MAX_PARTICLE_COUNT],
            vk::BufferUsageFlags::STORAGE_BUFFER | vk::BufferUsageFlags::VERTEX_BUFFER,
        );

        let (index_buffer, n_indices) = {
            let indices = [0u32, 1, 3, 3, 2, 0];
            (
                CommittedBuffer::upload(
                    &vk,
                    command_pool_transient.0,
                    &indices,
                    vk::BufferUsageFlags::INDEX_BUFFER,
                ),
                indices.len() as u32,
            )
        };

        let mut camera_data = CameraData::default();
        let camera_data_size = mem::size_of_val(&camera_data);

        let mut simulation_params = SimulationStepParams::default();
        let simulation_params_size = mem::size_of_val(&simulation_params);

        let command_buffers =
            vk.allocate_command_buffers(command_pool.0, MAX_CONCURRENT_FRAMES as _);
        let mut camera_buffers = Vec::with_capacity(MAX_CONCURRENT_FRAMES);
        let mut camera_mappings = Vec::with_capacity(MAX_CONCURRENT_FRAMES);
        let mut simulation_params_buffers = Vec::with_capacity(MAX_CONCURRENT_FRAMES);
        let mut simulation_params_mappings = Vec::with_capacity(MAX_CONCURRENT_FRAMES);
        let mut semaphores_image_available = Vec::with_capacity(MAX_CONCURRENT_FRAMES);
        let mut semaphores_render_finished = Vec::with_capacity(MAX_CONCURRENT_FRAMES);
        let mut fences_in_flight = Vec::with_capacity(MAX_CONCURRENT_FRAMES);
        for _ in 0..MAX_CONCURRENT_FRAMES {
            let buffer = CommittedBuffer::new(
                &vk,
                camera_data_size as _,
                vk::BufferUsageFlags::UNIFORM_BUFFER,
                vk::MemoryPropertyFlags::HOST_VISIBLE | vk::MemoryPropertyFlags::HOST_COHERENT,
            );
            let memory_mapping = vk
                .device
                .map_memory(
                    buffer.memory.0,
                    0,
                    camera_data_size as _,
                    vk::MemoryMapFlags::empty(),
                )
                .unwrap();
            camera_mappings.push(memory_mapping);
            camera_buffers.push(buffer);

            let buffer = CommittedBuffer::new(
                &vk,
                simulation_params_size as _,
                vk::BufferUsageFlags::UNIFORM_BUFFER,
                vk::MemoryPropertyFlags::HOST_VISIBLE | vk::MemoryPropertyFlags::HOST_COHERENT,
            );
            let memory_mapping = vk
                .device
                .map_memory(
                    buffer.memory.0,
                    0,
                    simulation_params_size as _,
                    vk::MemoryMapFlags::empty(),
                )
                .unwrap();
            simulation_params_mappings.push(memory_mapping);
            simulation_params_buffers.push(buffer);

            semaphores_image_available.push(vk.create_semaphore());
            semaphores_render_finished.push(vk.create_semaphore());
            fences_in_flight.push(vk.create_fence_signaled());
        }

        let descriptor_pool = create_descriptor_pool(&vk);
        let descriptor_sets_simulation = create_descriptor_sets_simulation(
            &vk,
            descriptor_pool.0,
            pipeline_simulate.descriptor_set_layout.0,
            &simulation_params_buffers,
            particles_buffer.buffer.0,
        );
        // let descriptor_sets_main = create_descriptor_sets_main(
        //     &vk,
        //     descriptor_pool.0,
        //     pipeline_main.descriptor_set_layout.0,
        //     &uniform_buffers,
        // );
        let descriptor_sets_particle = create_descriptor_sets_main(
            &vk,
            descriptor_pool.0,
            pipeline_particle.descriptor_set_layout.0,
            &camera_buffers,
        );
        let descriptor_sets_filter = create_descriptor_sets_filter(
            &vk,
            descriptor_pool.0,
            pipeline_filter.descriptor_set_layout.0,
        );

        let sampler = vk.create_sampler();
        update_descriptor_sets_filter(
            &vk,
            &descriptor_sets_filter,
            sampler.0,
            &swapchain.hdr_buffers,
        );

        let mut frame_in_flight_index = 0;

        let time_start = time::Instant::now();
        let mut time_prev = time_start;

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
                        update_descriptor_sets_filter(
                            &vk,
                            &descriptor_sets_filter,
                            sampler.0,
                            &swapchain.hdr_buffers,
                        );
                        continue 'main_loop;
                    }
                    _ => {}
                }
            }

            let time_curr = time::Instant::now();

            let ui = imgui.new_frame();
            ui.window("Info").build(|| {
                ui.text(format!("FPS: {}", ui.io().framerate));
                ui.input_float3("Orbit center", &mut state.orbit_center.0)
                    .build();
                ui.input_float2("Orbit distance", &mut state.orbit_distance.0)
                    .build();
                ui.slider("Turn speed", -360.0, 360.0, &mut state.turn_speed);
                ui.slider("Angle", -180.0, 180.0, &mut state.angle_deg);

                ui.spacing();

                ui.slider(
                    "Particle count",
                    0,
                    MAX_PARTICLE_COUNT as u32,
                    &mut state.particle_count,
                );
                ui.slider("Time scale", 0.0, 3.0, &mut state.time_scale);
                ui.slider("Particle lifetime", 0.0, 6.0, &mut state.init_ttl);
                ui.input_float4("Particle initial position", &mut state.init_pos.0)
                    .build();
                ui.input_float4("Particle initial velocity", &mut state.init_vel.0)
                    .build();
                ui.input_float4("Particle acceleration", &mut state.accel.0)
                    .build();

                ui.spacing();

                ui.slider("Blur radius", 0, 128, &mut state.blur_radius);
            });

            let time_elapsed = time_curr - time_prev;
            let nanos = time_elapsed.as_secs() * 1_000_000_000 + time_elapsed.subsec_nanos() as u64;
            state.update(nanos);
            time_prev = time_curr;

            let (sin, cos) = state.angle_deg.to_radians().sin_cos();

            let cam_pos = state.orbit_center
                + Vector([
                    sin * state.orbit_distance.x(),
                    state.orbit_distance.y(),
                    cos * state.orbit_distance.x(),
                ]);
            let look_at = state.orbit_center;
            let world_up = Vector([0.0, 1.0, 0.0]);
            let cam_forward = (look_at - cam_pos).normalize();
            let cam_right = cam_forward.cross(world_up).normalize();
            let cam_down = cam_forward.cross(cam_right);

            camera_data.mat_view.0[0][0] = cam_right.x();
            camera_data.mat_view.0[1][0] = cam_right.y();
            camera_data.mat_view.0[2][0] = cam_right.z();
            camera_data.mat_view.0[3][0] = -cam_right.dot(cam_pos);
            camera_data.mat_view.0[0][1] = cam_down.x();
            camera_data.mat_view.0[1][1] = cam_down.y();
            camera_data.mat_view.0[2][1] = cam_down.z();
            camera_data.mat_view.0[3][1] = -cam_down.dot(cam_pos);
            camera_data.mat_view.0[0][2] = cam_forward.x();
            camera_data.mat_view.0[1][2] = cam_forward.y();
            camera_data.mat_view.0[2][2] = cam_forward.z();
            camera_data.mat_view.0[3][2] = -cam_forward.dot(cam_pos);
            camera_data.mat_view.0[0][3] = 0.0;
            camera_data.mat_view.0[1][3] = 0.0;
            camera_data.mat_view.0[2][3] = 0.0;
            camera_data.mat_view.0[3][3] = 1.0;

            camera_data.mat_proj = mat4::identity();
            camera_data.mat_proj.0[0][0] =
                swapchain.extent.height as f32 / swapchain.extent.width as f32;
            camera_data.mat_proj.0[2][3] = 1.0;
            camera_data.mat_view_proj = camera_data.mat_proj.dot(&camera_data.mat_view);

            simulation_params.particle_count = state.particle_count;
            simulation_params.rng_seed = (time_curr - time_start).subsec_nanos();
            simulation_params.time_step = 1e-9 * nanos as f32 * state.time_scale;
            simulation_params.init_ttl = state.init_ttl;
            simulation_params.init_pos = state.init_pos;
            simulation_params.init_vel = state.init_vel;
            simulation_params.acc = state.accel;

            let cur_command_buffer = command_buffers[frame_in_flight_index];
            let cur_fence = fences_in_flight[frame_in_flight_index].0;
            let cur_image_available = semaphores_image_available[frame_in_flight_index].0;
            let cur_render_finished = semaphores_render_finished[frame_in_flight_index].0;
            // let cur_descriptor_set_main = descriptor_sets_main[frame_in_flight_index];
            let cur_descriptor_set_simulation = descriptor_sets_simulation[frame_in_flight_index];
            let cur_descriptor_set_particle = descriptor_sets_particle[frame_in_flight_index];

            ptr::copy(
                mem::transmute::<*const CameraData, *const std::ffi::c_void>(
                    &camera_data as *const _,
                ),
                camera_mappings[frame_in_flight_index],
                camera_data_size,
            );
            ptr::copy(
                mem::transmute::<*const SimulationStepParams, *const std::ffi::c_void>(
                    &simulation_params as *const _,
                ),
                simulation_params_mappings[frame_in_flight_index],
                simulation_params_size,
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
                    update_descriptor_sets_filter(
                        &vk,
                        &descriptor_sets_filter,
                        sampler.0,
                        &swapchain.hdr_buffers,
                    );
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
                vk::ClearValue::default(),
                vk::ClearValue {
                    depth_stencil: vk::ClearDepthStencilValue {
                        depth: 1.0,
                        stencil: 0,
                    },
                },
                vk::ClearValue {
                    color: vk::ClearColorValue {
                        // float32: [1.0, 0.75, 0.5, 0.0],
                        float32: [0.0; 4],
                    },
                },
                vk::ClearValue::default(),
                vk::ClearValue {
                    color: vk::ClearColorValue {
                        // float32: [1.0, 0.75, 0.5, 0.0],
                        float32: [0.0; 4],
                    },
                },
            ];
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

            vk.device.cmd_bind_pipeline(
                cur_command_buffer,
                vk::PipelineBindPoint::COMPUTE,
                pipeline_simulate.pipeline.0,
            );
            vk.device.cmd_bind_descriptor_sets(
                cur_command_buffer,
                vk::PipelineBindPoint::COMPUTE,
                pipeline_simulate.layout.0,
                0,
                &[cur_descriptor_set_simulation],
                &[],
            );
            vk.device
                .cmd_dispatch(cur_command_buffer, (state.particle_count + 255) / 256, 1, 1);

            let render_pass_begin = vk::RenderPassBeginInfo::default()
                .render_pass(render_pass.0)
                .framebuffer(swapchain.framebuffers[image_index as usize].0)
                .render_area(swapchain.extent.into())
                .clear_values(&clear_values);
            vk.device.cmd_begin_render_pass(
                cur_command_buffer,
                &render_pass_begin,
                vk::SubpassContents::INLINE,
            );
            vk.device.cmd_bind_pipeline(
                cur_command_buffer,
                vk::PipelineBindPoint::GRAPHICS,
                pipeline_particle.pipeline.0,
            );
            vk.device.cmd_bind_descriptor_sets(
                cur_command_buffer,
                vk::PipelineBindPoint::GRAPHICS,
                pipeline_particle.layout.0,
                0,
                &[cur_descriptor_set_particle],
                &[],
            );
            vk.device.cmd_bind_vertex_buffers(
                cur_command_buffer,
                0,
                &[particles_buffer.buffer.0],
                &[0],
            );
            vk.device.cmd_bind_index_buffer(
                cur_command_buffer,
                index_buffer.buffer.0,
                0,
                vk::IndexType::UINT32,
            );
            vk.device.cmd_draw_indexed(
                cur_command_buffer,
                n_indices,
                state.particle_count,
                0,
                0,
                0,
            );

            for i_filter in 0..4 {
                vk.device
                    .cmd_next_subpass(cur_command_buffer, vk::SubpassContents::INLINE);
                vk.device.cmd_bind_pipeline(
                    cur_command_buffer,
                    vk::PipelineBindPoint::GRAPHICS,
                    pipeline_filter.pipelines[i_filter].0,
                );
                vk.device.cmd_bind_descriptor_sets(
                    cur_command_buffer,
                    vk::PipelineBindPoint::GRAPHICS,
                    pipeline_filter.layout.0,
                    0,
                    &[descriptor_sets_filter[i_filter % 2]],
                    &[],
                );
                let push_constants = [
                    Vector([
                        swapchain.extent.width as f32,
                        swapchain.extent.height as f32,
                        1.0 / swapchain.extent.width as f32,
                        1.0 / swapchain.extent.height as f32,
                    ]),
                    Vector([state.blur_radius as f32, state.blur_radius as f32, 0.0, 0.0]),
                ];
                vk.device.cmd_push_constants(
                    cur_command_buffer,
                    pipeline_filter.layout.0,
                    vk::ShaderStageFlags::FRAGMENT,
                    0,
                    push_constants.align_to().1,
                );
                vk.device.cmd_draw(cur_command_buffer, 3, 1, 0, 0);
            }

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
                Ok(true) | Err(vk::Result::ERROR_OUT_OF_DATE_KHR) => {
                    swapchain.reinit();
                    update_descriptor_sets_filter(
                        &vk,
                        &descriptor_sets_filter,
                        sampler.0,
                        &swapchain.hdr_buffers,
                    );
                }
                Err(err) => panic!("Unexpected Vulkan error: {err}"),
            };

            frame_in_flight_index = (frame_in_flight_index + 1) % MAX_CONCURRENT_FRAMES;
        }

        vk.device.device_wait_idle().unwrap();
    }
}
