use crate::{
    vklib::{CommittedBuffer, VkContext},
    UniformData, MAX_CONCURRENT_FRAMES,
};
use ash::vk;
use std::slice;

pub unsafe fn create_descriptor_sets_main(
    vk: &VkContext,
    descriptor_pool: vk::DescriptorPool,
    layout: vk::DescriptorSetLayout,
    uniform_buffers: &[CommittedBuffer],
) -> Vec<vk::DescriptorSet> {
    let layouts = [layout; MAX_CONCURRENT_FRAMES];
    let allocate_info = vk::DescriptorSetAllocateInfo::default()
        .descriptor_pool(descriptor_pool)
        .set_layouts(&layouts);
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

pub unsafe fn create_descriptor_set_post_effect(
    vk: &VkContext,
    descriptor_pool: vk::DescriptorPool,
    layout: vk::DescriptorSetLayout,
) -> vk::DescriptorSet {
    let allocate_info = vk::DescriptorSetAllocateInfo::default()
        .descriptor_pool(descriptor_pool)
        .set_layouts(slice::from_ref(&layout));
    vk.device.allocate_descriptor_sets(&allocate_info).unwrap()[0]
}

pub unsafe fn update_descriptor_set_post_effect(
    vk: &VkContext,
    descriptor_set: vk::DescriptorSet,
    sampler: vk::Sampler,
    image_view: vk::ImageView,
) {
    let image_info = [vk::DescriptorImageInfo {
        sampler,
        image_view,
        image_layout: vk::ImageLayout::GENERAL,
    }];
    let descriptor_writes = [vk::WriteDescriptorSet::default()
        .dst_set(descriptor_set)
        .dst_binding(1)
        .descriptor_count(1)
        .descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER)
        .image_info(&image_info)];
    vk.device.update_descriptor_sets(&descriptor_writes, &[]);
}
