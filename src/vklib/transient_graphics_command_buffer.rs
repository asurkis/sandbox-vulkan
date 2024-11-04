use ash::vk;

use super::VkContext;

pub struct TransientGraphicsCommandBuffer<'a> {
    pub buffer: vk::CommandBuffer,
    pub pool: vk::CommandPool,
    vk: &'a VkContext,
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
