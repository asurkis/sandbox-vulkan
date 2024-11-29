use crate::{
    vklib::{vkbox, VkContext},
    MAX_CONCURRENT_FRAMES,
};
use ash::vk;

pub unsafe fn create_descriptor_pool(vk: &VkContext) -> vkbox::DescriptorPool {
    let pool_sizes = [
        vk::DescriptorPoolSize {
            ty: vk::DescriptorType::UNIFORM_BUFFER,
            descriptor_count: MAX_CONCURRENT_FRAMES as _,
        },
        vk::DescriptorPoolSize {
            ty: vk::DescriptorType::COMBINED_IMAGE_SAMPLER,
            descriptor_count: 1,
        },
    ];
    let create_info = vk::DescriptorPoolCreateInfo::default()
        .flags(vk::DescriptorPoolCreateFlags::empty())
        .max_sets(MAX_CONCURRENT_FRAMES as u32 + 1)
        .pool_sizes(&pool_sizes);
    vkbox::DescriptorPool::new(vk, &create_info)
}
