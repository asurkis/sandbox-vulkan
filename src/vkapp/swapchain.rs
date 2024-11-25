use crate::vklib::{VkContext, vkbox, CommittedImage};
use ash::vk;
use std::{mem, ptr};

pub struct Swapchain<'a> {
    _color_buffer: CommittedImage<'a>,
    _depth_buffer: CommittedImage<'a>,
    _image_views: Vec<vkbox::ImageView<'a>>,
    pub framebuffers: Vec<vkbox::Framebuffer<'a>>,
    pub swapchain: vkbox::SwapchainKHR<'a>,

    command_pool: vk::CommandPool,
    render_pass: vk::RenderPass,
    pub extent: vk::Extent2D,
    depth_buffer_format: vk::Format,
    samples: vk::SampleCountFlags,

    vk: &'a VkContext,
}

impl<'a> Swapchain<'a> {
    pub unsafe fn new(
        vk: &'a VkContext,
        command_pool: vk::CommandPool,
        render_pass: vk::RenderPass,
        depth_buffer_format: vk::Format,
        samples: vk::SampleCountFlags,
        old_swapchain: Option<&Self>,
    ) -> Self {
        let mut create_info = Self::create_info(vk);
        if let Some(old) = old_swapchain {
            create_info.old_swapchain = old.swapchain.0;
            vk.device.device_wait_idle().unwrap();
        }
        let swapchain = vkbox::SwapchainKHR::new(vk, &create_info);
        let images = vk
            .device_ext_swapchain
            .get_swapchain_images(swapchain.0)
            .unwrap();
        let image_views: Vec<_> = images
            .iter()
            .map(|&img| {
                vk.create_image_view(
                    img,
                    create_info.image_format,
                    vk::ImageAspectFlags::COLOR,
                    1,
                )
            })
            .collect();
        let msaa_on = samples != vk::SampleCountFlags::TYPE_1;
        let color_buffer = if msaa_on {
            CommittedImage::new(
                vk,
                create_info.image_format,
                create_info.image_extent,
                1,
                samples,
                vk::ImageUsageFlags::COLOR_ATTACHMENT,
                vk::ImageAspectFlags::COLOR,
            )
        } else {
            CommittedImage::default()
        };
        let depth_buffer_on = depth_buffer_format != vk::Format::UNDEFINED;
        let depth_buffer = if depth_buffer_on {
            vk.create_depth_buffer(
                command_pool,
                depth_buffer_format,
                create_info.image_extent,
                samples,
            )
        } else {
            CommittedImage::default()
        };
        let framebuffers: Vec<_> = image_views
            .iter()
            .map(|iv| {
                let attachments = if msaa_on {
                    vec![color_buffer.view.0, depth_buffer.view.0, iv.0]
                } else {
                    vec![iv.0, depth_buffer.view.0]
                };
                let extent = create_info.image_extent;
                let create_info = vk::FramebufferCreateInfo::default()
                    .render_pass(render_pass)
                    .attachments(&attachments)
                    .width(extent.width)
                    .height(extent.height)
                    .layers(1);
                vkbox::Framebuffer::new(vk, &create_info)
            })
            .collect();
        Self {
            framebuffers,
            _color_buffer: color_buffer,
            _depth_buffer: depth_buffer,
            _image_views: image_views,
            swapchain,
            command_pool,
            render_pass,
            extent: create_info.image_extent,
            depth_buffer_format,
            samples,
            vk,
        }
    }

    unsafe fn create_info(vk: &'a VkContext) -> vk::SwapchainCreateInfoKHR {
        let surface_capabilities = vk
            .instance_ext_surface
            .get_physical_device_surface_capabilities(
                vk.physical_device.physical_device,
                vk.surface,
            )
            .unwrap();
        let mut swapchain_create_info = vk::SwapchainCreateInfoKHR::default()
            .surface(vk.surface)
            .min_image_count(surface_capabilities.min_image_count)
            .image_format(vk.physical_device.surface_formats[0].format)
            .image_color_space(vk.physical_device.surface_formats[0].color_space)
            .image_extent(surface_capabilities.current_extent)
            .image_array_layers(1)
            .image_usage(vk::ImageUsageFlags::COLOR_ATTACHMENT)
            .image_sharing_mode(vk::SharingMode::CONCURRENT)
            .queue_family_indices(&vk.physical_device.queue_family_indices)
            .pre_transform(surface_capabilities.current_transform)
            .composite_alpha(vk::CompositeAlphaFlagsKHR::OPAQUE)
            .present_mode(vk::PresentModeKHR::FIFO)
            .clipped(false);
        if surface_capabilities.max_image_count == 0
            || surface_capabilities.min_image_count < surface_capabilities.max_image_count
        {
            swapchain_create_info.min_image_count += 1;
        }
        if vk.physical_device.queue_family_index_graphics
            == vk.physical_device.queue_family_index_present
        {
            swapchain_create_info.image_sharing_mode = vk::SharingMode::EXCLUSIVE;
            swapchain_create_info.queue_family_index_count = 0;
            swapchain_create_info.p_queue_family_indices = ptr::null();
        }
        for format in &vk.physical_device.surface_formats {
            if format.format == vk::Format::B8G8R8A8_UNORM
                && format.color_space == vk::ColorSpaceKHR::SRGB_NONLINEAR
            {
                swapchain_create_info.image_format = format.format;
                swapchain_create_info.image_color_space = format.color_space;
                break;
            }
        }
        for &present_mode in &vk.physical_device.surface_present_modes {
            if present_mode == vk::PresentModeKHR::MAILBOX {
                swapchain_create_info.present_mode = present_mode;
                break;
            }
        }
        swapchain_create_info
    }

    pub unsafe fn reinit(&mut self) {
        let capabilities = self
            .vk
            .instance_ext_surface
            .get_physical_device_surface_capabilities(
                self.vk.physical_device.physical_device,
                self.vk.surface,
            )
            .unwrap();
        let extent = capabilities.current_extent;
        if extent.width == 0 || extent.height == 0 {
            return;
        }
        let mut new = Self::new(
            self.vk,
            self.command_pool,
            self.render_pass,
            self.depth_buffer_format,
            self.samples,
            Some(self),
        );
        mem::swap(self, &mut new);
    }
}
