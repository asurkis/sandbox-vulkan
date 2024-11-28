use crate::{
    vklib::{vkbox, VkContext},
    Vertex,
};
use ash::vk;
use std::{ffi::CStr, mem, slice};

const BYTECODE_MAIN_VERT: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/main.vert.spv"));
const BYTECODE_MAIN_FRAG: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/main.frag.spv"));
const BYTECODE_POST_EFFECT_VERT: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/post_effect.vert.spv"));
const BYTECODE_POST_EFFECT_FRAG: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/post_effect.frag.spv"));

pub struct Pipelines<'a> {
    pub pipelines: [vkbox::Pipeline<'a>; 2],
    pub layouts: [vkbox::PipelineLayout<'a>; 2],
    pub descriptor_set_layouts: [vkbox::DescriptorSetLayout<'a>; 2],
}

impl<'a> Pipelines<'a> {
    pub unsafe fn new(
        vk: &'a VkContext,
        render_pass_scene: vk::RenderPass,
        render_pass_post_effect: vk::RenderPass,
        rasterization_samples: vk::SampleCountFlags,
    ) -> Self {
        let shader_module_main_vert = vk.create_shader_module(BYTECODE_MAIN_VERT);
        let shader_module_main_frag = vk.create_shader_module(BYTECODE_MAIN_FRAG);
        let shader_module_post_effect_vert = vk.create_shader_module(BYTECODE_POST_EFFECT_VERT);
        let shader_module_post_effect_frag = vk.create_shader_module(BYTECODE_POST_EFFECT_FRAG);

        let stage_create_infos = [
            [
                vk::PipelineShaderStageCreateInfo::default()
                    .stage(vk::ShaderStageFlags::VERTEX)
                    .module(shader_module_main_vert.0)
                    .name(CStr::from_bytes_with_nul(b"main\0").unwrap()),
                vk::PipelineShaderStageCreateInfo::default()
                    .stage(vk::ShaderStageFlags::FRAGMENT)
                    .module(shader_module_main_frag.0)
                    .name(CStr::from_bytes_with_nul(b"main\0").unwrap()),
            ],
            [
                vk::PipelineShaderStageCreateInfo::default()
                    .stage(vk::ShaderStageFlags::VERTEX)
                    .module(shader_module_post_effect_vert.0)
                    .name(CStr::from_bytes_with_nul(b"main\0").unwrap()),
                vk::PipelineShaderStageCreateInfo::default()
                    .stage(vk::ShaderStageFlags::FRAGMENT)
                    .module(shader_module_post_effect_frag.0)
                    .name(CStr::from_bytes_with_nul(b"main\0").unwrap()),
            ],
        ];
        let vertex_binding_descriptions = [vk::VertexInputBindingDescription {
            binding: 0,
            stride: mem::size_of::<Vertex>() as _,
            input_rate: vk::VertexInputRate::VERTEX,
        }];
        let vertex_attribute_descriptions = [
            vk::VertexInputAttributeDescription {
                location: 0,
                binding: 0,
                format: vk::Format::R32G32B32_SFLOAT,
                offset: mem::offset_of!(Vertex, pos) as _,
            },
            vk::VertexInputAttributeDescription {
                location: 1,
                binding: 0,
                format: vk::Format::R32G32B32_SFLOAT,
                offset: mem::offset_of!(Vertex, norm) as _,
            },
        ];
        let vertex_input_state = [
            vk::PipelineVertexInputStateCreateInfo::default()
                .vertex_binding_descriptions(&vertex_binding_descriptions)
                .vertex_attribute_descriptions(&vertex_attribute_descriptions),
            vk::PipelineVertexInputStateCreateInfo::default()
                .vertex_binding_descriptions(&[])
                .vertex_attribute_descriptions(&[]),
        ];
        let input_assembly_state = vk::PipelineInputAssemblyStateCreateInfo::default()
            .topology(vk::PrimitiveTopology::TRIANGLE_LIST);
        let viewports = [vk::Viewport::default()];
        let scissors = [vk::Rect2D::default()];
        let viewport_state = vk::PipelineViewportStateCreateInfo::default()
            .viewports(&viewports)
            .scissors(&scissors);
        let rasterization_state = vk::PipelineRasterizationStateCreateInfo::default()
            .polygon_mode(vk::PolygonMode::FILL)
            .cull_mode(vk::CullModeFlags::BACK)
            .front_face(vk::FrontFace::COUNTER_CLOCKWISE)
            .line_width(1.0);
        let multisample_state = [
            vk::PipelineMultisampleStateCreateInfo::default()
                .rasterization_samples(rasterization_samples),
            vk::PipelineMultisampleStateCreateInfo::default()
                .rasterization_samples(vk::SampleCountFlags::TYPE_1),
        ];
        let color_blend_attachments = [
            [vk::PipelineColorBlendAttachmentState {
                blend_enable: vk::TRUE,
                src_color_blend_factor: vk::BlendFactor::ONE,
                dst_color_blend_factor: vk::BlendFactor::ZERO,
                color_blend_op: vk::BlendOp::ADD,
                src_alpha_blend_factor: vk::BlendFactor::ONE,
                dst_alpha_blend_factor: vk::BlendFactor::ZERO,
                alpha_blend_op: vk::BlendOp::ADD,
                color_write_mask: vk::ColorComponentFlags::A
                    | vk::ColorComponentFlags::B
                    | vk::ColorComponentFlags::G
                    | vk::ColorComponentFlags::R,
            }],
            [vk::PipelineColorBlendAttachmentState {
                blend_enable: vk::FALSE,
                src_color_blend_factor: vk::BlendFactor::ONE,
                dst_color_blend_factor: vk::BlendFactor::ZERO,
                color_blend_op: vk::BlendOp::ADD,
                src_alpha_blend_factor: vk::BlendFactor::ONE,
                dst_alpha_blend_factor: vk::BlendFactor::ZERO,
                alpha_blend_op: vk::BlendOp::ADD,
                color_write_mask: vk::ColorComponentFlags::B
                    | vk::ColorComponentFlags::G
                    | vk::ColorComponentFlags::R,
            }],
        ];
        let depth_stencil_state = vk::PipelineDepthStencilStateCreateInfo::default()
            .flags(vk::PipelineDepthStencilStateCreateFlags::empty())
            .depth_test_enable(true)
            .depth_write_enable(true)
            .depth_compare_op(vk::CompareOp::LESS)
            .depth_bounds_test_enable(false)
            .stencil_test_enable(false);
        let color_blend_state = [
            vk::PipelineColorBlendStateCreateInfo::default()
                .attachments(&color_blend_attachments[0]),
            vk::PipelineColorBlendStateCreateInfo::default()
                .attachments(&color_blend_attachments[1]),
        ];
        let dynamic_states = [vk::DynamicState::VIEWPORT, vk::DynamicState::SCISSOR];
        let dynamic_state_create_info =
            vk::PipelineDynamicStateCreateInfo::default().dynamic_states(&dynamic_states);

        let bindings = [
            [vk::DescriptorSetLayoutBinding::default()
                .binding(0)
                .descriptor_type(vk::DescriptorType::UNIFORM_BUFFER)
                .descriptor_count(1)
                .stage_flags(vk::ShaderStageFlags::VERTEX)],
            [vk::DescriptorSetLayoutBinding::default()
                .binding(1)
                .descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER)
                .descriptor_count(1)
                .stage_flags(vk::ShaderStageFlags::FRAGMENT)],
        ];
        let descriptor_set_layout_create_infos = [
            vk::DescriptorSetLayoutCreateInfo::default().bindings(&bindings[0]),
            vk::DescriptorSetLayoutCreateInfo::default().bindings(&bindings[1]),
        ];
        let descriptor_set_layouts = [
            vkbox::DescriptorSetLayout::new(vk, &descriptor_set_layout_create_infos[0]),
            vkbox::DescriptorSetLayout::new(vk, &descriptor_set_layout_create_infos[1]),
        ];

        let layout_create_infos = [
            vk::PipelineLayoutCreateInfo::default()
                .set_layouts(slice::from_ref(&descriptor_set_layouts[0].0)),
            vk::PipelineLayoutCreateInfo::default()
                .set_layouts(slice::from_ref(&descriptor_set_layouts[1].0)),
        ];
        let layouts = [
            vkbox::PipelineLayout::new(vk, &layout_create_infos[0]),
            vkbox::PipelineLayout::new(vk, &layout_create_infos[1]),
        ];
        let pipeline_create_infos = [
            vk::GraphicsPipelineCreateInfo::default()
                .stages(&stage_create_infos[0])
                .vertex_input_state(&vertex_input_state[0])
                .input_assembly_state(&input_assembly_state)
                .viewport_state(&viewport_state)
                .rasterization_state(&rasterization_state)
                .multisample_state(&multisample_state[0])
                .depth_stencil_state(&depth_stencil_state)
                .color_blend_state(&color_blend_state[0])
                .dynamic_state(&dynamic_state_create_info)
                .layout(layouts[0].0)
                .render_pass(render_pass_scene)
                .subpass(0),
            vk::GraphicsPipelineCreateInfo::default()
                .stages(&stage_create_infos[1])
                .vertex_input_state(&vertex_input_state[1])
                .input_assembly_state(&input_assembly_state)
                .viewport_state(&viewport_state)
                .rasterization_state(&rasterization_state)
                .multisample_state(&multisample_state[1])
                .depth_stencil_state(&depth_stencil_state)
                .color_blend_state(&color_blend_state[1])
                .dynamic_state(&dynamic_state_create_info)
                .layout(layouts[1].0)
                .render_pass(render_pass_post_effect)
                .subpass(0),
        ];
        let pipelines = vk
            .device
            .create_graphics_pipelines(vk::PipelineCache::null(), &pipeline_create_infos, None)
            .unwrap();

        Self {
            pipelines: [
                vkbox::Pipeline::wrap(vk, pipelines[0]),
                vkbox::Pipeline::wrap(vk, pipelines[1]),
            ],
            layouts,
            descriptor_set_layouts,
        }
    }
}
