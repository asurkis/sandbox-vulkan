use crate::vklib::{vkbox, VkContext};
use ash::vk;

pub unsafe fn create_render_pass(
    vk: &VkContext,
    hdr_format: vk::Format,
    depth_buffer_format: vk::Format,
    samples: vk::SampleCountFlags,
) -> vkbox::RenderPass {
    let display_format = vk.physical_device.surface_formats[0].format;
    let msaa_on = samples != vk::SampleCountFlags::TYPE_1;
    let attachments = [
        // [0] swapchain image
        vk::AttachmentDescription {
            flags: vk::AttachmentDescriptionFlags::empty(),
            format: display_format,
            samples: vk::SampleCountFlags::TYPE_1,
            load_op: vk::AttachmentLoadOp::DONT_CARE,
            store_op: vk::AttachmentStoreOp::STORE,
            stencil_load_op: vk::AttachmentLoadOp::DONT_CARE,
            stencil_store_op: vk::AttachmentStoreOp::DONT_CARE,
            initial_layout: vk::ImageLayout::UNDEFINED,
            final_layout: vk::ImageLayout::PRESENT_SRC_KHR,
        },
        // [1] depth buffer
        vk::AttachmentDescription {
            flags: vk::AttachmentDescriptionFlags::empty(),
            format: depth_buffer_format,
            samples,
            load_op: vk::AttachmentLoadOp::CLEAR,
            store_op: vk::AttachmentStoreOp::DONT_CARE,
            stencil_load_op: vk::AttachmentLoadOp::CLEAR,
            stencil_store_op: vk::AttachmentStoreOp::DONT_CARE,
            initial_layout: vk::ImageLayout::UNDEFINED,
            final_layout: vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL,
        },
        // [2] hdr buffer 1
        vk::AttachmentDescription {
            flags: vk::AttachmentDescriptionFlags::empty(),
            format: hdr_format,
            samples: vk::SampleCountFlags::TYPE_1,
            load_op: if msaa_on {
                vk::AttachmentLoadOp::DONT_CARE
            } else {
                vk::AttachmentLoadOp::CLEAR
            },
            store_op: vk::AttachmentStoreOp::STORE,
            stencil_load_op: vk::AttachmentLoadOp::DONT_CARE,
            stencil_store_op: vk::AttachmentStoreOp::DONT_CARE,
            initial_layout: vk::ImageLayout::GENERAL,
            final_layout: vk::ImageLayout::GENERAL,
        },
        // [3] hdr buffer 2
        vk::AttachmentDescription {
            flags: vk::AttachmentDescriptionFlags::empty(),
            format: hdr_format,
            samples: vk::SampleCountFlags::TYPE_1,
            load_op: vk::AttachmentLoadOp::DONT_CARE,
            store_op: vk::AttachmentStoreOp::STORE,
            stencil_load_op: vk::AttachmentLoadOp::DONT_CARE,
            stencil_store_op: vk::AttachmentStoreOp::DONT_CARE,
            initial_layout: vk::ImageLayout::GENERAL,
            final_layout: vk::ImageLayout::GENERAL,
        },
        // [4] msaa buffer
        vk::AttachmentDescription {
            flags: vk::AttachmentDescriptionFlags::empty(),
            format: hdr_format,
            samples,
            load_op: vk::AttachmentLoadOp::CLEAR,
            store_op: vk::AttachmentStoreOp::STORE,
            stencil_load_op: vk::AttachmentLoadOp::DONT_CARE,
            stencil_store_op: vk::AttachmentStoreOp::DONT_CARE,
            initial_layout: vk::ImageLayout::UNDEFINED,
            final_layout: vk::ImageLayout::GENERAL,
        },
    ];
    let color_attachments_sp0 = [vk::AttachmentReference {
        attachment: if msaa_on { 4 } else { 2 },
        layout: vk::ImageLayout::GENERAL,
    }];
    let color_attachments_filter0 = [
        vk::AttachmentReference {
            attachment: 0,
            layout: vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
        },
        vk::AttachmentReference {
            attachment: 3,
            layout: vk::ImageLayout::GENERAL,
        },
    ];
    let color_attachments_filter1 = [vk::AttachmentReference {
        attachment: 2,
        layout: vk::ImageLayout::GENERAL,
    }];
    let color_attachments_filter2 = [vk::AttachmentReference {
        attachment: 3,
        layout: vk::ImageLayout::GENERAL,
    }];
    let color_attachments_filter3 = [vk::AttachmentReference {
        attachment: 0,
        layout: vk::ImageLayout::GENERAL,
    }];
    let depth_stencil_attachment = vk::AttachmentReference {
        attachment: 1,
        layout: vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL,
    };
    let resolve_attachments = [vk::AttachmentReference {
        attachment: 2,
        layout: vk::ImageLayout::GENERAL,
    }];
    let subpasses = [
        {
            let mut base = vk::SubpassDescription::default()
                .pipeline_bind_point(vk::PipelineBindPoint::GRAPHICS)
                .color_attachments(&color_attachments_sp0)
                .depth_stencil_attachment(&depth_stencil_attachment);
            if msaa_on {
                base = base.resolve_attachments(&resolve_attachments)
            }
            base
        },
        vk::SubpassDescription::default()
            .pipeline_bind_point(vk::PipelineBindPoint::GRAPHICS)
            .color_attachments(&color_attachments_filter0),
        vk::SubpassDescription::default()
            .pipeline_bind_point(vk::PipelineBindPoint::GRAPHICS)
            .color_attachments(&color_attachments_filter1),
        vk::SubpassDescription::default()
            .pipeline_bind_point(vk::PipelineBindPoint::GRAPHICS)
            .color_attachments(&color_attachments_filter2),
        vk::SubpassDescription::default()
            .pipeline_bind_point(vk::PipelineBindPoint::GRAPHICS)
            .color_attachments(&color_attachments_filter3),
    ];
    let dependencies = [
        vk::SubpassDependency {
            src_subpass: vk::SUBPASS_EXTERNAL,
            dst_subpass: 0,
            src_stage_mask: vk::PipelineStageFlags::FRAGMENT_SHADER,
            dst_stage_mask: vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT,
            src_access_mask: vk::AccessFlags::SHADER_READ,
            dst_access_mask: vk::AccessFlags::COLOR_ATTACHMENT_WRITE,
            dependency_flags: vk::DependencyFlags::empty(),
        },
        vk::SubpassDependency {
            src_subpass: vk::SUBPASS_EXTERNAL,
            dst_subpass: 1,
            src_stage_mask: vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT,
            dst_stage_mask: vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT,
            src_access_mask: vk::AccessFlags::COLOR_ATTACHMENT_WRITE,
            dst_access_mask: vk::AccessFlags::COLOR_ATTACHMENT_WRITE,
            dependency_flags: vk::DependencyFlags::empty(),
        },
        vk::SubpassDependency {
            src_subpass: 0,
            dst_subpass: 1,
            src_stage_mask: vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT,
            dst_stage_mask: vk::PipelineStageFlags::FRAGMENT_SHADER,
            src_access_mask: vk::AccessFlags::COLOR_ATTACHMENT_WRITE,
            dst_access_mask: vk::AccessFlags::SHADER_READ,
            dependency_flags: vk::DependencyFlags::empty(),
        },
        vk::SubpassDependency {
            src_subpass: 1,
            dst_subpass: 2,
            src_stage_mask: vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT,
            dst_stage_mask: vk::PipelineStageFlags::FRAGMENT_SHADER,
            src_access_mask: vk::AccessFlags::COLOR_ATTACHMENT_WRITE,
            dst_access_mask: vk::AccessFlags::SHADER_READ,
            dependency_flags: vk::DependencyFlags::empty(),
        },
        vk::SubpassDependency {
            src_subpass: 2,
            dst_subpass: 3,
            src_stage_mask: vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT,
            dst_stage_mask: vk::PipelineStageFlags::FRAGMENT_SHADER,
            src_access_mask: vk::AccessFlags::COLOR_ATTACHMENT_WRITE,
            dst_access_mask: vk::AccessFlags::SHADER_READ,
            dependency_flags: vk::DependencyFlags::empty(),
        },
        vk::SubpassDependency {
            src_subpass: 3,
            dst_subpass: 4,
            src_stage_mask: vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT,
            dst_stage_mask: vk::PipelineStageFlags::FRAGMENT_SHADER,
            src_access_mask: vk::AccessFlags::COLOR_ATTACHMENT_WRITE,
            dst_access_mask: vk::AccessFlags::SHADER_READ,
            dependency_flags: vk::DependencyFlags::empty(),
        },
    ];
    let create_info = vk::RenderPassCreateInfo::default()
        .attachments(&attachments[..4 + msaa_on as usize])
        .subpasses(&subpasses)
        .dependencies(&dependencies);
    vkbox::RenderPass::new(vk, &create_info)
}
