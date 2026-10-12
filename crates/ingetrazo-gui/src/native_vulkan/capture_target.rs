//! Temporary output attachment for the existing host painter; no second renderer.
use super::{annotation_stencil, budget::VulkanBudget, world::Image, VulkanContext};
use ash::{vk, Device};
use std::sync::Arc;

pub(super) struct Target {
    device: Device,
    lifetime: Arc<super::lifetime::Lifetime>,
    pub color: Option<Image>,
    stencil: Option<annotation_stencil::Target>,
    pub pass: vk::RenderPass,
    pub framebuffer: vk::Framebuffer,
    pub extent: vk::Extent2D,
    pub format: vk::Format,
}
impl Target {
    pub fn new(
        context: &VulkanContext,
        budget: Arc<VulkanBudget>,
        extent: vk::Extent2D,
        format: vk::Format,
        integer: bool,
        stencil: bool,
    ) -> Result<Self, String> {
        context.lifetime.check()?;
        if !(64..=4096).contains(&extent.width) || !(64..=4096).contains(&extent.height) {
            return Err("Capture extent must be64..4096".into());
        }
        let mut out = Self {
            device: context.device.clone(),
            lifetime: context.lifetime.clone(),
            color: None,
            stencil: None,
            pass: vk::RenderPass::null(),
            framebuffer: vk::Framebuffer::null(),
            extent,
            format,
        };
        let usage = vk::ImageUsageFlags::COLOR_ATTACHMENT
            | vk::ImageUsageFlags::TRANSFER_SRC
            | vk::ImageUsageFlags::SAMPLED
            | if integer {
                vk::ImageUsageFlags::INPUT_ATTACHMENT
            } else {
                vk::ImageUsageFlags::empty()
            };
        out.color = Some(Image::new(
            context,
            budget.clone(),
            extent,
            format,
            usage,
            vk::ImageAspectFlags::COLOR,
        )?);
        if stencil {
            out.stencil = Some(annotation_stencil::Target::new(context, budget, extent)?);
        }
        // Attachment formats/sample counts/subpass are identical to the accepted
        // host pass. Only the final layout differs, which preserves compatibility.
        out.pass = super::create_render_pass_with_final_layout(
            &out.device,
            format,
            integer,
            out.stencil.as_ref().map(|s| s.format),
            vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
        )
        .map_err(|e| e.to_string())?;
        let mut views = vec![out.color.as_ref().unwrap().view()];
        if let Some(stencil) = &out.stencil {
            views.push(stencil.view);
        }
        out.framebuffer = unsafe {
            out.device.create_framebuffer(
                &vk::FramebufferCreateInfo::default()
                    .render_pass(out.pass)
                    .attachments(&views)
                    .width(extent.width)
                    .height(extent.height)
                    .layers(1),
                None,
            )
        }
        .map_err(|e| e.to_string())?;
        Ok(out)
    }
    pub fn view(&self) -> vk::ImageView {
        self.color.as_ref().unwrap().view()
    }
    pub fn image(&self) -> vk::Image {
        self.color.as_ref().unwrap().handle()
    }
}
impl Drop for Target {
    fn drop(&mut self) {
        if self.lifetime.uncertain() {
            // Submission ownership is unknown. Keep attachments and Vulkan handles
            // alive with the already quarantined device, never free in-flight data.
            if let Some(color) = self.color.take() {
                self.lifetime.retain(color);
            }
            if let Some(stencil) = self.stencil.take() {
                self.lifetime.retain(stencil);
            }
            return;
        }
        unsafe {
            if self.framebuffer != vk::Framebuffer::null() {
                self.device.destroy_framebuffer(self.framebuffer, None);
            }
            if self.pass != vk::RenderPass::null() {
                self.device.destroy_render_pass(self.pass, None);
            }
        }
    }
}
