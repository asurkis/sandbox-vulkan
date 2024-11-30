use crate::{
    vklib::{vkbox, VkContext},
    Particle, Vertex,
};
use ash::vk;
use std::{array, ffi::CStr, mem, slice};

const BYTECODE_MAIN_VERT: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/main.vert.spv"));
const BYTECODE_MAIN_FRAG: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/main.frag.spv"));
const BYTECODE_PARTICLE_VERT: &[u8] =
    include_bytes!(concat!(env!("OUT_DIR"), "/particle.vert.spv"));
const BYTECODE_PARTICLE_FRAG: &[u8] =
    include_bytes!(concat!(env!("OUT_DIR"), "/particle.frag.spv"));
const BYTECODE_FILTER_VERT: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/filter.vert.spv"));
const BYTECODE_FILTER_FRAG: [&[u8]; 4] = [
    include_bytes!(concat!(env!("OUT_DIR"), "/filter0.frag.spv")),
    include_bytes!(concat!(env!("OUT_DIR"), "/filter1.frag.spv")),
    include_bytes!(concat!(env!("OUT_DIR"), "/filter2.frag.spv")),
    include_bytes!(concat!(env!("OUT_DIR"), "/filter3.frag.spv")),
];
const BYTECODE_SIMULATION: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/simulation.comp.spv"));

pub struct PipelineBox<'a> {
    pub pipeline: vkbox::Pipeline<'a>,
    pub layout: vkbox::PipelineLayout<'a>,
    pub descriptor_set_layout: vkbox::DescriptorSetLayout<'a>,
}

pub struct PipelineVec<'a> {
    pub pipelines: Vec<vkbox::Pipeline<'a>>,
    pub layout: vkbox::PipelineLayout<'a>,
    pub descriptor_set_layout: vkbox::DescriptorSetLayout<'a>,
}

impl<'a> PipelineBox<'a> {
    pub unsafe fn new_main(
        vk: &'a VkContext,
        render_pass: vk::RenderPass,
        rasterization_samples: vk::SampleCountFlags,
    ) -> Self {
        let shader_module_main_vert = vk.create_shader_module(BYTECODE_MAIN_VERT);
        let shader_module_main_frag = vk.create_shader_module(BYTECODE_MAIN_FRAG);

        let stage_create_infos = [
            vk::PipelineShaderStageCreateInfo::default()
                .stage(vk::ShaderStageFlags::VERTEX)
                .module(shader_module_main_vert.0)
                .name(CStr::from_bytes_with_nul(b"main\0").unwrap()),
            vk::PipelineShaderStageCreateInfo::default()
                .stage(vk::ShaderStageFlags::FRAGMENT)
                .module(shader_module_main_frag.0)
                .name(CStr::from_bytes_with_nul(b"main\0").unwrap()),
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
        let vertex_input_state = vk::PipelineVertexInputStateCreateInfo::default()
            .vertex_binding_descriptions(&vertex_binding_descriptions)
            .vertex_attribute_descriptions(&vertex_attribute_descriptions);
        let input_assembly_state = vk::PipelineInputAssemblyStateCreateInfo::default()
            .topology(vk::PrimitiveTopology::TRIANGLE_LIST);
        let viewports = [vk::Viewport::default()];
        let scissors = [vk::Rect2D::default()];
        let viewport_state = vk::PipelineViewportStateCreateInfo::default()
            .viewports(&viewports)
            .scissors(&scissors);
        let rasterization_state = vk::PipelineRasterizationStateCreateInfo::default()
            .polygon_mode(vk::PolygonMode::FILL)
            // .cull_mode(vk::CullModeFlags::BACK)
            .front_face(vk::FrontFace::COUNTER_CLOCKWISE)
            .line_width(1.0);
        let multisample_state = vk::PipelineMultisampleStateCreateInfo::default()
            .rasterization_samples(rasterization_samples);
        let color_blend_attachments = [vk::PipelineColorBlendAttachmentState {
            blend_enable: vk::TRUE,
            src_color_blend_factor: vk::BlendFactor::ONE,
            dst_color_blend_factor: vk::BlendFactor::ONE_MINUS_SRC_ALPHA,
            color_blend_op: vk::BlendOp::ADD,
            src_alpha_blend_factor: vk::BlendFactor::ONE,
            dst_alpha_blend_factor: vk::BlendFactor::ZERO,
            alpha_blend_op: vk::BlendOp::ADD,
            color_write_mask: vk::ColorComponentFlags::A
                | vk::ColorComponentFlags::B
                | vk::ColorComponentFlags::G
                | vk::ColorComponentFlags::R,
        }];
        let depth_stencil_state = vk::PipelineDepthStencilStateCreateInfo::default()
            .flags(vk::PipelineDepthStencilStateCreateFlags::empty())
            // .depth_test_enable(true)
            // .depth_write_enable(true)
            // .depth_compare_op(vk::CompareOp::LESS)
            .depth_bounds_test_enable(false)
            .stencil_test_enable(false);
        let color_blend_state =
            vk::PipelineColorBlendStateCreateInfo::default().attachments(&color_blend_attachments);
        let dynamic_states = [vk::DynamicState::VIEWPORT, vk::DynamicState::SCISSOR];
        let dynamic_state_create_info =
            vk::PipelineDynamicStateCreateInfo::default().dynamic_states(&dynamic_states);

        let bindings = [vk::DescriptorSetLayoutBinding::default()
            .binding(0)
            .descriptor_type(vk::DescriptorType::UNIFORM_BUFFER)
            .descriptor_count(1)
            .stage_flags(vk::ShaderStageFlags::VERTEX)];
        let descriptor_set_layout_create_info =
            vk::DescriptorSetLayoutCreateInfo::default().bindings(&bindings);
        let descriptor_set_layout =
            vkbox::DescriptorSetLayout::new(vk, &descriptor_set_layout_create_info);

        let layout_create_info = vk::PipelineLayoutCreateInfo::default()
            .set_layouts(slice::from_ref(&descriptor_set_layout.0));
        let layout = vkbox::PipelineLayout::new(vk, &layout_create_info);
        let pipeline_create_infos = [vk::GraphicsPipelineCreateInfo::default()
            .stages(&stage_create_infos)
            .vertex_input_state(&vertex_input_state)
            .input_assembly_state(&input_assembly_state)
            .viewport_state(&viewport_state)
            .rasterization_state(&rasterization_state)
            .multisample_state(&multisample_state)
            .depth_stencil_state(&depth_stencil_state)
            .color_blend_state(&color_blend_state)
            .dynamic_state(&dynamic_state_create_info)
            .layout(layout.0)
            .render_pass(render_pass)
            .subpass(0)];
        let pipelines = vk
            .device
            .create_graphics_pipelines(vk::PipelineCache::null(), &pipeline_create_infos, None)
            .unwrap();

        Self {
            pipeline: vkbox::Pipeline::wrap(vk, pipelines[0]),
            layout,
            descriptor_set_layout,
        }
    }

    pub unsafe fn new_particle(
        vk: &'a VkContext,
        render_pass: vk::RenderPass,
        rasterization_samples: vk::SampleCountFlags,
    ) -> Self {
        let shader_module_particle_vert = vk.create_shader_module(BYTECODE_PARTICLE_VERT);
        let shader_module_particle_frag = vk.create_shader_module(BYTECODE_PARTICLE_FRAG);

        let stage_create_infos = [
            vk::PipelineShaderStageCreateInfo::default()
                .stage(vk::ShaderStageFlags::VERTEX)
                .module(shader_module_particle_vert.0)
                .name(CStr::from_bytes_with_nul(b"main\0").unwrap()),
            vk::PipelineShaderStageCreateInfo::default()
                .stage(vk::ShaderStageFlags::FRAGMENT)
                .module(shader_module_particle_frag.0)
                .name(CStr::from_bytes_with_nul(b"main\0").unwrap()),
        ];
        let vertex_binding_descriptions = [vk::VertexInputBindingDescription {
            binding: 0,
            stride: mem::size_of::<Particle>() as _,
            input_rate: vk::VertexInputRate::INSTANCE,
        }];
        let vertex_attribute_descriptions = [
            vk::VertexInputAttributeDescription {
                location: 0,
                binding: 0,
                format: vk::Format::R32G32B32A32_SFLOAT,
                offset: mem::offset_of!(Particle, pos) as _,
            },
            vk::VertexInputAttributeDescription {
                location: 1,
                binding: 0,
                format: vk::Format::R32G32B32A32_SFLOAT,
                offset: mem::offset_of!(Particle, vel) as _,
            },
        ];
        let vertex_input_state = vk::PipelineVertexInputStateCreateInfo::default()
            .vertex_binding_descriptions(&vertex_binding_descriptions)
            .vertex_attribute_descriptions(&vertex_attribute_descriptions);
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
        let multisample_state = vk::PipelineMultisampleStateCreateInfo::default()
            .rasterization_samples(rasterization_samples);
        let color_blend_attachments = [vk::PipelineColorBlendAttachmentState {
            blend_enable: vk::TRUE,
            src_color_blend_factor: vk::BlendFactor::ONE,
            dst_color_blend_factor: vk::BlendFactor::ONE_MINUS_SRC_ALPHA,
            color_blend_op: vk::BlendOp::ADD,
            src_alpha_blend_factor: vk::BlendFactor::ONE,
            dst_alpha_blend_factor: vk::BlendFactor::ZERO,
            alpha_blend_op: vk::BlendOp::ADD,
            color_write_mask: vk::ColorComponentFlags::A
                | vk::ColorComponentFlags::B
                | vk::ColorComponentFlags::G
                | vk::ColorComponentFlags::R,
        }];
        let depth_stencil_state = vk::PipelineDepthStencilStateCreateInfo::default()
            .flags(vk::PipelineDepthStencilStateCreateFlags::empty())
            .depth_test_enable(true)
            // .depth_write_enable(true)
            .depth_compare_op(vk::CompareOp::LESS)
            .depth_bounds_test_enable(false)
            .stencil_test_enable(false);
        let color_blend_state =
            vk::PipelineColorBlendStateCreateInfo::default().attachments(&color_blend_attachments);
        let dynamic_states = [vk::DynamicState::VIEWPORT, vk::DynamicState::SCISSOR];
        let dynamic_state_create_info =
            vk::PipelineDynamicStateCreateInfo::default().dynamic_states(&dynamic_states);

        let bindings = [vk::DescriptorSetLayoutBinding::default()
            .binding(0)
            .descriptor_type(vk::DescriptorType::UNIFORM_BUFFER)
            .descriptor_count(1)
            .stage_flags(vk::ShaderStageFlags::VERTEX)];
        let descriptor_set_layout_create_info =
            vk::DescriptorSetLayoutCreateInfo::default().bindings(&bindings);
        let descriptor_set_layout =
            vkbox::DescriptorSetLayout::new(vk, &descriptor_set_layout_create_info);

        let layout_create_info = vk::PipelineLayoutCreateInfo::default()
            .set_layouts(slice::from_ref(&descriptor_set_layout.0));
        let layout = vkbox::PipelineLayout::new(vk, &layout_create_info);
        let pipeline_create_infos = [vk::GraphicsPipelineCreateInfo::default()
            .stages(&stage_create_infos)
            .vertex_input_state(&vertex_input_state)
            .input_assembly_state(&input_assembly_state)
            .viewport_state(&viewport_state)
            .rasterization_state(&rasterization_state)
            .multisample_state(&multisample_state)
            .depth_stencil_state(&depth_stencil_state)
            .color_blend_state(&color_blend_state)
            .dynamic_state(&dynamic_state_create_info)
            .layout(layout.0)
            .render_pass(render_pass)
            .subpass(0)];
        let pipelines = vk
            .device
            .create_graphics_pipelines(vk::PipelineCache::null(), &pipeline_create_infos, None)
            .unwrap();

        Self {
            pipeline: vkbox::Pipeline::wrap(vk, pipelines[0]),
            layout,
            descriptor_set_layout,
        }
    }

    pub unsafe fn new_simulation(vk: &'a VkContext) -> Self {
        let shader_module_stage_simulation = vk.create_shader_module(BYTECODE_SIMULATION);

        let stage_create_info = vk::PipelineShaderStageCreateInfo::default()
            .stage(vk::ShaderStageFlags::COMPUTE)
            .module(shader_module_stage_simulation.0)
            .name(CStr::from_bytes_with_nul(b"main\0").unwrap());

        let bindings = [
            vk::DescriptorSetLayoutBinding::default()
                .binding(0)
                .descriptor_type(vk::DescriptorType::UNIFORM_BUFFER)
                .descriptor_count(1)
                .stage_flags(vk::ShaderStageFlags::COMPUTE),
            vk::DescriptorSetLayoutBinding::default()
                .binding(1)
                .descriptor_type(vk::DescriptorType::STORAGE_BUFFER)
                .descriptor_count(1)
                .stage_flags(vk::ShaderStageFlags::COMPUTE),
        ];

        let descriptor_set_layout_create_info =
            vk::DescriptorSetLayoutCreateInfo::default().bindings(&bindings);
        let descriptor_set_layout =
            vkbox::DescriptorSetLayout::new(vk, &descriptor_set_layout_create_info);
        let layout_create_info = vk::PipelineLayoutCreateInfo::default()
            .set_layouts(slice::from_ref(&descriptor_set_layout.0));
        let layout = vkbox::PipelineLayout::new(vk, &layout_create_info);

        let pipeline_create_infos = [vk::ComputePipelineCreateInfo::default()
            .stage(stage_create_info)
            .layout(layout.0)];
        let pipelines = vk
            .device
            .create_compute_pipelines(vk::PipelineCache::null(), &pipeline_create_infos, None)
            .unwrap();
        Self {
            pipeline: vkbox::Pipeline::wrap(vk, pipelines[0]),
            layout,
            descriptor_set_layout,
        }
    }
}

impl<'a> PipelineVec<'a> {
    pub unsafe fn new_filters(vk: &'a VkContext, render_pass: vk::RenderPass) -> Self {
        let shader_module_filter_vert = vk.create_shader_module(BYTECODE_FILTER_VERT);
        let shader_module_filter_frag: [_; 4] =
            array::from_fn(|i| vk.create_shader_module(BYTECODE_FILTER_FRAG[i]));
        let stage_create_infos: [_; 4] = array::from_fn(|i| {
            [
                vk::PipelineShaderStageCreateInfo::default()
                    .stage(vk::ShaderStageFlags::VERTEX)
                    .module(shader_module_filter_vert.0)
                    .name(CStr::from_bytes_with_nul(b"main\0").unwrap()),
                vk::PipelineShaderStageCreateInfo::default()
                    .stage(vk::ShaderStageFlags::FRAGMENT)
                    .module(shader_module_filter_frag[i].0)
                    .name(CStr::from_bytes_with_nul(b"main\0").unwrap()),
            ]
        });
        let vertex_input_state = vk::PipelineVertexInputStateCreateInfo::default()
            .vertex_binding_descriptions(&[])
            .vertex_attribute_descriptions(&[]);
        let input_assembly_state = vk::PipelineInputAssemblyStateCreateInfo::default()
            .topology(vk::PrimitiveTopology::TRIANGLE_LIST);
        let viewports = [vk::Viewport::default()];
        let scissors = [vk::Rect2D::default()];
        let viewport_state = vk::PipelineViewportStateCreateInfo::default()
            .viewports(&viewports)
            .scissors(&scissors);
        let rasterization_state = vk::PipelineRasterizationStateCreateInfo::default()
            .polygon_mode(vk::PolygonMode::FILL)
            .cull_mode(vk::CullModeFlags::NONE)
            // .front_face(vk::FrontFace::COUNTER_CLOCKWISE)
            .line_width(1.0);
        let multisample_state = vk::PipelineMultisampleStateCreateInfo::default()
            .rasterization_samples(vk::SampleCountFlags::TYPE_1);
        let color_blend_attachments = [vk::PipelineColorBlendAttachmentState {
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
        }; 2];
        let depth_stencil_state = vk::PipelineDepthStencilStateCreateInfo::default();
        let color_blend_state = [
            vk::PipelineColorBlendStateCreateInfo::default().attachments(&color_blend_attachments),
            vk::PipelineColorBlendStateCreateInfo::default()
                .attachments(&color_blend_attachments[..1]),
        ];
        let dynamic_states = [vk::DynamicState::VIEWPORT, vk::DynamicState::SCISSOR];
        let dynamic_state_create_info =
            vk::PipelineDynamicStateCreateInfo::default().dynamic_states(&dynamic_states);

        let bindings = [vk::DescriptorSetLayoutBinding::default()
            .binding(1)
            .descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER)
            .descriptor_count(1)
            .stage_flags(vk::ShaderStageFlags::FRAGMENT)];
        let descriptor_set_layout_create_info =
            vk::DescriptorSetLayoutCreateInfo::default().bindings(&bindings);
        let descriptor_set_layout =
            vkbox::DescriptorSetLayout::new(vk, &descriptor_set_layout_create_info);

        let push_constant_ranges = [vk::PushConstantRange {
            stage_flags: vk::ShaderStageFlags::FRAGMENT,
            offset: 0,
            size: 32,
        }];

        let layout_create_info = vk::PipelineLayoutCreateInfo::default()
            .set_layouts(slice::from_ref(&descriptor_set_layout.0))
            .push_constant_ranges(&push_constant_ranges);
        let layout = vkbox::PipelineLayout::new(vk, &layout_create_info);
        let pipeline_create_infos: [_; 4] = array::from_fn(|i| {
            vk::GraphicsPipelineCreateInfo::default()
                .stages(&stage_create_infos[i])
                .vertex_input_state(&vertex_input_state)
                .input_assembly_state(&input_assembly_state)
                .viewport_state(&viewport_state)
                .rasterization_state(&rasterization_state)
                .multisample_state(&multisample_state)
                .depth_stencil_state(&depth_stencil_state)
                .color_blend_state(&color_blend_state[i.min(1)])
                .dynamic_state(&dynamic_state_create_info)
                .layout(layout.0)
                .render_pass(render_pass)
                .subpass(1 + i as u32)
        });
        let pipelines = vk
            .device
            .create_graphics_pipelines(vk::PipelineCache::null(), &pipeline_create_infos, None)
            .unwrap();

        Self {
            pipelines: pipelines
                .iter()
                .map(|&x| vkbox::Pipeline::wrap(vk, x))
                .collect(),
            layout,
            descriptor_set_layout,
        }
    }
}
