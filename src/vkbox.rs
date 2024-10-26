macro_rules! declare_box {
    ($typ:ident, $device:ident, $create_info_ty:ident, $create_fn:ident, $destroy_fn:ident) => {
        #[derive(Default)]
        pub struct $typ<'a>(pub ::ash::vk::$typ, Option<&'a crate::bootstrap::VkContext>);

        impl<'a> $typ<'a> {
            #[allow(unused)]
            pub unsafe fn new(
                vk: &'a crate::bootstrap::VkContext,
                create_info: &::ash::vk::$create_info_ty,
            ) -> Self {
                Self(vk.$device.$create_fn(create_info, None).unwrap(), Some(vk))
            }

            #[allow(unused)]
            pub fn wrap(vk: &'a crate::bootstrap::VkContext, x: ::ash::vk::$typ) -> Self {
                Self(x, Some(vk))
            }

            #[allow(unused)]
            pub fn null() -> Self {
                Self(::ash::vk::$typ::null(), None)
            }
        }

        impl Drop for $typ<'_> {
            fn drop(&mut self) {
                match self.1 {
                    Some(vk) => unsafe {
                        vk.$device.$destroy_fn(self.0, None);
                    },
                    None => {}
                }
            }
        }

        impl ::std::fmt::Debug for $typ<'_> {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                self.0.fmt(f)
            }
        }
    };
}

declare_box!(Fence, device, FenceCreateInfo, create_fence, destroy_fence);
declare_box!(
    Semaphore,
    device,
    SemaphoreCreateInfo,
    create_semaphore,
    destroy_semaphore
);
declare_box!(
    Buffer,
    device,
    BufferCreateInfo,
    create_buffer,
    destroy_buffer
);
declare_box!(Image, device, ImageCreateInfo, create_image, destroy_image);
declare_box!(
    CommandPool,
    device,
    CommandPoolCreateInfo,
    create_command_pool,
    destroy_command_pool
);
declare_box!(
    RenderPass,
    device,
    RenderPassCreateInfo,
    create_render_pass,
    destroy_render_pass
);
declare_box!(
    ShaderModule,
    device,
    ShaderModuleCreateInfo,
    create_shader_module,
    destroy_shader_module
);
declare_box!(
    ImageView,
    device,
    ImageViewCreateInfo,
    create_image_view,
    destroy_image_view
);
declare_box!(
    Framebuffer,
    device,
    FramebufferCreateInfo,
    create_framebuffer,
    destroy_framebuffer
);
declare_box!(
    DescriptorSetLayout,
    device,
    DescriptorSetLayoutCreateInfo,
    create_descriptor_set_layout,
    destroy_descriptor_set_layout
);
declare_box!(
    PipelineLayout,
    device,
    PipelineLayoutCreateInfo,
    create_pipeline_layout,
    destroy_pipeline_layout
);
declare_box!(
    DescriptorPool,
    device,
    DescriptorPoolCreateInfo,
    create_descriptor_pool,
    destroy_descriptor_pool
);
declare_box!(
    Sampler,
    device,
    SamplerCreateInfo,
    create_sampler,
    destroy_sampler
);

declare_box!(
    DeviceMemory,
    device,
    MemoryAllocateInfo,
    allocate_memory,
    free_memory
);

declare_box!(
    SwapchainKHR,
    device_ext_swapchain,
    SwapchainCreateInfoKHR,
    create_swapchain,
    destroy_swapchain
);
