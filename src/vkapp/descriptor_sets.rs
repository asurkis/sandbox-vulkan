use crate::{
    vklib::{vkbox, CommittedBuffer, CommittedImage, VkContext},
    CameraData, SimulationStepParams, MAX_CONCURRENT_FRAMES,
};
use ash::vk;
use std::{array, slice};

pub unsafe fn create_descriptor_sets_simulation(
    vk: &VkContext,
    descriptor_pool: vk::DescriptorPool,
    layout: vk::DescriptorSetLayout,
    params_buffers: &[CommittedBuffer],
    particles_buffer: vk::Buffer,
) -> Vec<vk::DescriptorSet> {
    let set_layouts = [layout; MAX_CONCURRENT_FRAMES];
    let allocate_info = vk::DescriptorSetAllocateInfo::default()
        .descriptor_pool(descriptor_pool)
        .set_layouts(&set_layouts);
    let sets = vk.device.allocate_descriptor_sets(&allocate_info).unwrap();
    for i in 0..MAX_CONCURRENT_FRAMES {
        let uniform_buffer_info = [vk::DescriptorBufferInfo {
            buffer: params_buffers[i].buffer.0,
            offset: 0,
            range: std::mem::size_of::<SimulationStepParams>() as _,
        }];
        let storage_buffer_info = [vk::DescriptorBufferInfo {
            buffer: particles_buffer,
            offset: 0,
            range: vk::WHOLE_SIZE,
        }];
        let descriptor_writes = [
            vk::WriteDescriptorSet::default()
                .dst_set(sets[i])
                .dst_binding(0)
                .descriptor_count(1)
                .descriptor_type(vk::DescriptorType::UNIFORM_BUFFER)
                .buffer_info(&uniform_buffer_info),
            vk::WriteDescriptorSet::default()
                .dst_set(sets[i])
                .dst_binding(1)
                .descriptor_count(1)
                .descriptor_type(vk::DescriptorType::STORAGE_BUFFER)
                .buffer_info(&storage_buffer_info),
        ];
        vk.device.update_descriptor_sets(&descriptor_writes, &[]);
    }
    sets
}

pub unsafe fn create_descriptor_sets_main(
    vk: &VkContext,
    descriptor_pool: vk::DescriptorPool,
    layout: vk::DescriptorSetLayout,
    camera_buffers: &[CommittedBuffer],
) -> Vec<vk::DescriptorSet> {
    let set_layouts = [layout; MAX_CONCURRENT_FRAMES];
    let allocate_info = vk::DescriptorSetAllocateInfo::default()
        .descriptor_pool(descriptor_pool)
        .set_layouts(&set_layouts);
    let sets = vk.device.allocate_descriptor_sets(&allocate_info).unwrap();
    for i in 0..MAX_CONCURRENT_FRAMES {
        let uniform_buffer_info = [vk::DescriptorBufferInfo {
            buffer: camera_buffers[i].buffer.0,
            offset: 0,
            range: std::mem::size_of::<CameraData>() as _,
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

pub unsafe fn create_descriptor_sets_filter(
    vk: &VkContext,
    descriptor_pool: vk::DescriptorPool,
    layout: vk::DescriptorSetLayout,
) -> Vec<vk::DescriptorSet> {
    let set_layouts = [layout; 2];
    let allocate_info = vk::DescriptorSetAllocateInfo::default()
        .descriptor_pool(descriptor_pool)
        .set_layouts(&set_layouts);
    vk.device.allocate_descriptor_sets(&allocate_info).unwrap()
}

pub unsafe fn update_descriptor_sets_filter(
    vk: &VkContext,
    descriptor_set: &[vk::DescriptorSet],
    sampler: vk::Sampler,
    hdr_buffers: &[CommittedImage; 2],
) {
    let image_info: [_; 2] = array::from_fn(|i| {
        [vk::DescriptorImageInfo {
            sampler,
            image_view: hdr_buffers[i].view.0,
            image_layout: vk::ImageLayout::GENERAL,
        }]
    });
    let descriptor_writes: [_; 2] = array::from_fn(|i| {
        vk::WriteDescriptorSet::default()
            .dst_set(descriptor_set[i])
            .dst_binding(1)
            .dst_array_element(0)
            .descriptor_count(1)
            .descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER)
            .image_info(&image_info[i])
    });
    vk.device.update_descriptor_sets(&descriptor_writes, &[]);
}
