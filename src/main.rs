mod math;
mod state;
mod vkapp;
mod vklib;
mod voxel;

use ash::vk::{self, BufferUsageFlags};
use math::{mat4, vec3, Vector};
use sdl2::event::Event;
use state::StateBox;
use std::{mem, ptr, slice, time};
use vkapp::{
    create_descriptor_pool, create_descriptor_sets, create_render_pass, Pipelines, Swapchain,
};
use vklib::{CommittedBuffer, SdlContext, VkContext};
use voxel::octree::Octree;

const MAX_CONCURRENT_FRAMES: usize = 2;

#[derive(Clone, Copy, Debug, Default)]
struct UniformData {
    mat_view: mat4,
    mat_proj: mat4,
    mat_view_proj: mat4,
}

#[derive(Clone, Copy, Debug, Default)]
struct Vertex {
    pos: vec3,
    norm: vec3,
}

fn main() {
    let (indices, vertices) = {
        const LOG_RADIUS: i32 = 5;
        const RADIUS: i32 = 1 << LOG_RADIUS;
        const DIAMETER: i32 = 2 * RADIUS;
        let mut voxels = vec![0; (DIAMETER * DIAMETER * DIAMETER) as _];
        for z in 0..DIAMETER {
            for y in 0..DIAMETER {
                for x in 0..DIAMETER {
                    let i_vox = ((z * DIAMETER + y) * DIAMETER + x) as usize;
                    let dx = 2 * x + 1 - 2 * RADIUS;
                    let dy = 2 * y + 1 - 2 * RADIUS;
                    let dz = 2 * z + 1 - 2 * RADIUS;
                    let r2 = dx * dx + dy * dy + dz * dz;
                    if r2 < 4 * RADIUS * RADIUS {
                        voxels[i_vox] = 0;
                    } else {
                        voxels[i_vox] = !0;
                    }
                }
            }
        }
        Octree::from_voxels(&voxels).debug_mesh()
    };

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
        let pipelines = Pipelines::new(&vk, render_pass.0, msaa_sample_count);

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
                subpass: 1,
                sample_count: vk::SampleCountFlags::TYPE_1,
            }),
        )
        .unwrap();

        let vertex_buffer = CommittedBuffer::upload(
            &vk,
            command_pool_transient.0,
            &vertices,
            BufferUsageFlags::VERTEX_BUFFER,
        );
        let index_buffer = CommittedBuffer::upload(
            &vk,
            command_pool_transient.0,
            &indices,
            BufferUsageFlags::INDEX_BUFFER,
        );

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
                uniform_data_size as _,
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
            &pipelines.descriptor_set_layouts,
            &uniform_buffers,
        );

        let sampler = vk.create_sampler();

        for i in 0..MAX_CONCURRENT_FRAMES {
            let image_info = [vk::DescriptorImageInfo {
                sampler: sampler.0,
                image_view: swapchain.hdr_buffer.view.0,
                image_layout: vk::ImageLayout::GENERAL,
            }];
            let descriptor_writes = [vk::WriteDescriptorSet::default()
                .dst_set(descriptor_sets[MAX_CONCURRENT_FRAMES + i])
                .dst_binding(1)
                .descriptor_count(1)
                .descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER)
                .image_info(&image_info)];
            vk.device.update_descriptor_sets(&descriptor_writes, &[]);
        }

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

                        for i in 0..MAX_CONCURRENT_FRAMES {
                            let image_info = [vk::DescriptorImageInfo {
                                sampler: sampler.0,
                                image_view: swapchain.hdr_buffer.view.0,
                                image_layout: vk::ImageLayout::GENERAL,
                            }];
                            let descriptor_writes = [vk::WriteDescriptorSet::default()
                                .dst_set(descriptor_sets[MAX_CONCURRENT_FRAMES + i])
                                .dst_binding(1)
                                .descriptor_count(1)
                                .descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER)
                                .image_info(&image_info)];
                            vk.device.update_descriptor_sets(&descriptor_writes, &[]);
                        }
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

            uniform_data.mat_view.0[0][0] = cam_right.x();
            uniform_data.mat_view.0[1][0] = cam_right.y();
            uniform_data.mat_view.0[2][0] = cam_right.z();
            uniform_data.mat_view.0[3][0] = -cam_right.dot(cam_pos);
            uniform_data.mat_view.0[0][1] = cam_down.x();
            uniform_data.mat_view.0[1][1] = cam_down.y();
            uniform_data.mat_view.0[2][1] = cam_down.z();
            uniform_data.mat_view.0[3][1] = -cam_down.dot(cam_pos);
            uniform_data.mat_view.0[0][2] = cam_forward.x();
            uniform_data.mat_view.0[1][2] = cam_forward.y();
            uniform_data.mat_view.0[2][2] = cam_forward.z();
            uniform_data.mat_view.0[3][2] = -cam_forward.dot(cam_pos);
            uniform_data.mat_view.0[0][3] = 0.0;
            uniform_data.mat_view.0[1][3] = 0.0;
            uniform_data.mat_view.0[2][3] = 0.0;
            uniform_data.mat_view.0[3][3] = 1.0;

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
            let cur_descriptor_sets = [
                descriptor_sets[frame_in_flight_index],
                descriptor_sets[MAX_CONCURRENT_FRAMES + frame_in_flight_index],
            ];

            ptr::copy(
                mem::transmute::<*const UniformData, *const std::ffi::c_void>(
                    &uniform_data as *const _,
                ),
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

                    for i in 0..MAX_CONCURRENT_FRAMES {
                        let image_info = [vk::DescriptorImageInfo {
                            sampler: sampler.0,
                            image_view: swapchain.hdr_buffer.view.0,
                            image_layout: vk::ImageLayout::GENERAL,
                        }];
                        let descriptor_writes = [vk::WriteDescriptorSet::default()
                            .dst_set(descriptor_sets[MAX_CONCURRENT_FRAMES + i])
                            .dst_binding(1)
                            .descriptor_count(1)
                            .descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER)
                            .image_info(&image_info)];
                        vk.device.update_descriptor_sets(&descriptor_writes, &[]);
                    }
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
                        // float32: [1.0, 0.75, 0.5, 0.0],
                        float32: [0.0; 4],
                    },
                },
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
                pipelines.pipelines[0].0,
            );
            vk.device.cmd_bind_descriptor_sets(
                cur_command_buffer,
                vk::PipelineBindPoint::GRAPHICS,
                pipelines.layouts[0].0,
                0,
                slice::from_ref(&cur_descriptor_sets[0]),
                &[],
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
            vk.device
                .cmd_draw_indexed(cur_command_buffer, indices.len() as _, 1, 0, 0, 0);

            vk.device
                .cmd_next_subpass(cur_command_buffer, vk::SubpassContents::INLINE);
            vk.device.cmd_bind_pipeline(
                cur_command_buffer,
                vk::PipelineBindPoint::GRAPHICS,
                pipelines.pipelines[1].0,
            );
            vk.device.cmd_bind_descriptor_sets(
                cur_command_buffer,
                vk::PipelineBindPoint::GRAPHICS,
                pipelines.layouts[1].0,
                0,
                slice::from_ref(&cur_descriptor_sets[1]),
                &[],
            );
            vk.device.cmd_draw(cur_command_buffer, 3, 1, 0, 0);

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

                    for i in 0..MAX_CONCURRENT_FRAMES {
                        let image_info = [vk::DescriptorImageInfo {
                            sampler: sampler.0,
                            image_view: swapchain.hdr_buffer.view.0,
                            image_layout: vk::ImageLayout::GENERAL,
                        }];
                        let descriptor_writes = [vk::WriteDescriptorSet::default()
                            .dst_set(descriptor_sets[MAX_CONCURRENT_FRAMES + i])
                            .dst_binding(1)
                            .descriptor_count(1)
                            .descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER)
                            .image_info(&image_info)];
                        vk.device.update_descriptor_sets(&descriptor_writes, &[]);
                    }
                }
                Err(err) => panic!("Unexpected Vulkan error: {err}"),
            };

            frame_in_flight_index = (frame_in_flight_index + 1) % MAX_CONCURRENT_FRAMES;
        }

        vk.device.device_wait_idle().unwrap();
    }
}
