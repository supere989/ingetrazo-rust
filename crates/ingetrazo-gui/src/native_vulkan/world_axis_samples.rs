//! Diagnostic-only precise per-sample query counts; never writes attachments.
use ash::{vk, Device};
pub(super) const SITES: [[i32; 2]; 4] = [[575, 1], [749, 441], [1, 639], [825, 227]];
const COUNT: u32 = 97;
pub(super) struct Owner {
    device: Device,
    pool: vk::QueryPool,
    pipelines: [vk::Pipeline; 9],
    recorded: bool,
}
impl Owner {
    pub(super) fn new(
        device: &Device,
        pass: vk::RenderPass,
        layout: vk::PipelineLayout,
        samples: vk::SampleCountFlags,
        source_depth: bool,
    ) -> Result<Self, String> {
        if samples != vk::SampleCountFlags::TYPE_4 {
            return Err("Axis query requires actual four samples".into());
        }
        let pool = unsafe {
            device.create_query_pool(
                &vk::QueryPoolCreateInfo::default()
                    .query_type(vk::QueryType::OCCLUSION)
                    .query_count(COUNT),
                None,
            )
        }
        .map_err(|e| e.to_string())?;
        let mut owner = Self {
            device: device.clone(),
            pool,
            pipelines: [vk::Pipeline::null(); 9],
            recorded: false,
        };
        for sample in 0..4 {
            for enabled in [true, false] {
                let index = sample * 2 + usize::from(!enabled);
                owner.pipelines[index] = super::make_pipeline_source_depth_query(
                    device,
                    pass,
                    layout,
                    super::WorldPass::Axes,
                    samples,
                    false,
                    Some(&[1u32 << sample]),
                    false,
                    source_depth,
                    Some(enabled),
                )?;
            }
        }
        owner.pipelines[8] = super::make_pipeline_source_depth_query(
            device,
            pass,
            layout,
            super::WorldPass::Axes,
            samples,
            false,
            Some(&[0xf]),
            false,
            source_depth,
            Some(false),
        )?;
        Ok(owner)
    }
    pub(super) fn reset(&mut self, command: vk::CommandBuffer) {
        unsafe {
            self.device
                .cmd_reset_query_pool(command, self.pool, 0, COUNT);
        }
        self.recorded = false;
    }
    pub(super) fn record(&mut self, command: vk::CommandBuffer, extent: vk::Extent2D) {
        let mut query = 0;
        unsafe {
            self.device.cmd_set_scissor(
                command,
                0,
                &[vk::Rect2D {
                    offset: vk::Offset2D::default(),
                    extent,
                }],
            );
            self.device.cmd_bind_pipeline(
                command,
                vk::PipelineBindPoint::GRAPHICS,
                self.pipelines[8],
            );
            self.device
                .cmd_begin_query(command, self.pool, 96, vk::QueryControlFlags::PRECISE);
            self.device.cmd_draw(command, 3126, 1, 0, 0);
            self.device.cmd_end_query(command, self.pool, 96);
            for axis in 0..3 {
                for [x, top_y] in SITES {
                    self.device.cmd_set_scissor(
                        command,
                        0,
                        &[vk::Rect2D {
                            offset: vk::Offset2D {
                                x,
                                y: extent.height as i32 - 1 - top_y,
                            },
                            extent: vk::Extent2D {
                                width: 1,
                                height: 1,
                            },
                        }],
                    );
                    for sample in 0..4 {
                        for enabled in [true, false] {
                            self.device.cmd_bind_pipeline(
                                command,
                                vk::PipelineBindPoint::GRAPHICS,
                                self.pipelines[sample * 2 + usize::from(!enabled)],
                            );
                            self.device.cmd_begin_query(
                                command,
                                self.pool,
                                query,
                                vk::QueryControlFlags::PRECISE,
                            );
                            self.device.cmd_draw(command, 1042, 1, axis * 1042, 0);
                            self.device.cmd_end_query(command, self.pool, query);
                            query += 1;
                        }
                    }
                }
            }
            self.device.cmd_set_scissor(
                command,
                0,
                &[vk::Rect2D {
                    offset: vk::Offset2D::default(),
                    extent,
                }],
            );
        }
        self.recorded = true;
    }
    pub(super) fn report(&self) -> Result<serde_json::Value, String> {
        if !self.recorded {
            return Err("Actual requested-target axes query not recorded".into());
        }
        let mut values = [0u64; COUNT as usize];
        unsafe {
            self.device.get_query_pool_results(
                self.pool,
                0,
                &mut values,
                vk::QueryResultFlags::TYPE_64,
            )
        }
        .map_err(|e| e.to_string())?;
        let mut rows = Vec::new();
        rows.try_reserve_exact(48)
            .map_err(|_| "Axis query report admission")?;
        let mut i = 0;
        for axis in 0..3 {
            for [x, top_y] in SITES {
                for sample in 0..4 {
                    rows.push(serde_json::json!({"site_top_origin":[x,top_y],"attachment_pixel":[x,767-top_y],"axis_first":axis*1042,"sample":sample,"counts":{"depth_enabled":values[i],"depth_disabled":values[i+1]}}));
                    i += 2;
                }
            }
        }
        let positive = values[96] > 0;
        Ok(
            serde_json::json!({"positive_control_passed":positive,"positive_control_scope":"Full viewport, all four samples, actual 3126-vertex axes draw, depth disabled; counts only","full_viewport_depth_disabled_count":values[96],"diagnostic_short_dash_site":[825,227],"scope":"Precise per-sample occlusion counts before ordinary axes draw; not raw stored color/depth","requested_extent":[1024,768],"color_writes":false,"depth_writes":false,"precise_feature_admitted":true,"rows":rows}),
        )
    }
}
impl Drop for Owner {
    fn drop(&mut self) {
        unsafe {
            for pipeline in self.pipelines {
                if pipeline != vk::Pipeline::null() {
                    self.device.destroy_pipeline(pipeline, None);
                }
            }
            self.device.destroy_query_pool(self.pool, None);
        }
    }
}
