use crate::{
    vklib::{CommittedBuffer, VkContext},
    UniformData, MAX_CONCURRENT_FRAMES,
};
use ash::vk;

pub unsafe fn create_descriptor_sets(
    vk: &VkContext,
    descriptor_pool: vk::DescriptorPool,
    layout: vk::DescriptorSetLayout,
    uniform_buffers: &[CommittedBuffer],
    storage_buffer: vk::Buffer,
) -> Vec<vk::DescriptorSet> {
    let set_layouts = [layout; MAX_CONCURRENT_FRAMES];
    let allocate_info = vk::DescriptorSetAllocateInfo::default()
        .descriptor_pool(descriptor_pool)
        .set_layouts(&set_layouts);
    let sets = vk.device.allocate_descriptor_sets(&allocate_info).unwrap();
    for i in 0..MAX_CONCURRENT_FRAMES {
        let uniform_buffer_info = [vk::DescriptorBufferInfo {
            buffer: uniform_buffers[i].buffer.0,
            offset: 0,
            range: std::mem::size_of::<UniformData>() as _,
        }];
        let storage_buffer_info = [vk::DescriptorBufferInfo {
            buffer: storage_buffer,
            offset: 0,
            range: vk::WHOLE_SIZE,
        }];
        let descriptor_writes = [
            vk::WriteDescriptorSet::default()
                .dst_set(sets[i])
                .dst_binding(0)
                .dst_array_element(0)
                .descriptor_type(vk::DescriptorType::UNIFORM_BUFFER)
                .buffer_info(&uniform_buffer_info),
            vk::WriteDescriptorSet::default()
                .dst_set(sets[i])
                .dst_binding(1)
                .dst_array_element(0)
                .descriptor_type(vk::DescriptorType::STORAGE_BUFFER)
                .buffer_info(&storage_buffer_info),
        ];
        vk.device.update_descriptor_sets(&descriptor_writes, &[]);
    }
    sets
}
