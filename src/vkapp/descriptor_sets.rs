use crate::{
    vklib::{vkbox, CommittedBuffer, VkContext},
    UniformData, MAX_CONCURRENT_FRAMES,
};
use ash::vk;

pub unsafe fn create_descriptor_sets(
    vk: &VkContext,
    descriptor_pool: vk::DescriptorPool,
    layouts: &[vkbox::DescriptorSetLayout],
    uniform_buffers: &[CommittedBuffer],
) -> Vec<vk::DescriptorSet> {
    let mut set_layouts = Vec::with_capacity(layouts.len() * MAX_CONCURRENT_FRAMES);
    for layout in layouts {
        for _ in 0..MAX_CONCURRENT_FRAMES {
            set_layouts.push(layout.0);
        }
    }
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
        let descriptor_writes = [vk::WriteDescriptorSet::default()
            .dst_set(sets[i])
            .dst_binding(0)
            .dst_array_element(0)
            .descriptor_type(vk::DescriptorType::UNIFORM_BUFFER)
            .buffer_info(&uniform_buffer_info)];
        vk.device.update_descriptor_sets(&descriptor_writes, &[]);
    }
    sets
}
