use super::{vkbox, TransientGraphicsCommandBuffer, VkContext};
use ash::vk;
use std::{mem, ptr};

#[derive(Debug, Default)]
pub struct CommittedBuffer<'a> {
    pub buffer: vkbox::Buffer<'a>,
    pub memory: vkbox::DeviceMemory<'a>,
}

impl<'a> CommittedBuffer<'a> {
    pub unsafe fn new(
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
        let buffer = vkbox::Buffer::new(vk, &create_info);

        let memory_requirements = vk.device.get_buffer_memory_requirements(buffer.0);
        let memory = vk.allocate_memory(memory_requirements, memory_property_flags);
        vk.device.bind_buffer_memory(buffer.0, memory.0, 0).unwrap();
        Self { buffer, memory }
    }

    pub unsafe fn new_staging<T: Copy>(vk: &'a VkContext, data: &[T]) -> Self {
        let data_size = mem::size_of_val(data);
        let staging = Self::new(
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
        let out = Self::new(
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
