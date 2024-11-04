use crate::vklib::{CommittedBuffer, TransientGraphicsCommandBuffer};

use super::{vkbox, VkContext};
use ash::vk;

#[derive(Debug, Default)]
pub struct CommittedImage<'a> {
    pub image: vkbox::Image<'a>,
    pub view: vkbox::ImageView<'a>,
    pub _memory: vkbox::DeviceMemory<'a>,
}

impl<'a> CommittedImage<'a> {
    pub unsafe fn new(
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
        let image = vkbox::Image::new(vk, &create_info);
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
        let out = Self::new(
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
