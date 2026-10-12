// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Marco Sumari Tellez and IngeTrazo contributors.

pub mod arc;
pub mod base;
pub mod circle;
mod shape_plane;
pub mod change_axes;
pub mod dimension;
pub mod eraser;
pub mod fillet;
pub mod followme;
pub mod freehand;
pub mod line;
pub mod move_tool;
pub mod offset;
pub mod paint;
pub mod place_group;
pub mod image;
pub mod polygon;
pub mod protractor;
pub mod pushpull;
pub mod rectangle;
pub mod rotate_tool;
pub mod rotated_rectangle;
pub mod scale_tool;
pub mod section;
pub mod select;
mod select_pick;
pub mod tape;
pub mod text;

pub use base::*;

pub use arc::{ArcMode, ArcTool};
pub use circle::CircleTool;
pub use change_axes::ChangeAxesTool;
pub use dimension::DimensionTool;
pub use eraser::EraserTool;
pub use fillet::FilletTool;
pub use followme::FollowMeTool;
pub use freehand::FreehandTool;
pub use line::LineTool;
pub use move_tool::MoveTool;
pub use offset::OffsetTool;
pub use paint::PaintTool;
pub use place_group::PlaceGroupTool;
pub use image::ImageTool;
pub use polygon::PolygonTool;
pub use protractor::ProtractorTool;
pub use pushpull::PushPullTool;
pub use rectangle::RectangleTool;
pub use rotate_tool::RotateTool;
pub use rotated_rectangle::RotatedRectangleTool;
pub use scale_tool::ScaleTool;
pub use section::SectionPlaneTool;
pub use select::SelectTool;
pub use tape::{TapeMode, TapeTool};
pub use text::TextTool;

#[cfg(test)]
mod tests {
    use super::*;
    use glam::DVec3;
    use ingetrazo_core::math::Ray;
    use ingetrazo_core::scene::Scene;

    #[test]
    fn test_create_tools() {
        for tool_type in [
            ToolType::Select,
            ToolType::Line,
            ToolType::Freehand,
            ToolType::Rectangle,
            ToolType::RotatedRectangle,
            ToolType::Circle,
            ToolType::Polygon,
            ToolType::Arc,
            ToolType::PushPull,
            ToolType::Offset,
            ToolType::Move,
            ToolType::Rotate,
            ToolType::Scale,
            ToolType::Tape,
            ToolType::Eraser,
            ToolType::Paint,
            ToolType::Fillet,
            ToolType::FollowMe,
            ToolType::Protractor,
            ToolType::Dimension,
            ToolType::Text,
            ToolType::SectionPlane,
            ToolType::PlaceGroup,
        ] {
            let tool = create_tool(tool_type);
            assert_eq!(tool.tool_type(), tool_type);
            assert!(!tool.name().is_empty());
        }
    }

    #[test]
    fn test_line_tool_drawing() {
        let mut scene = Scene::new();
        let mut line_tool = line::LineTool::default();

        let ctx0 = ToolContext {
            screen: None,
            world: DVec3::new(0.0, 0.0, 0.0),
            ray: Ray::new(DVec3::new(0.0, 0.0, 10.0), -DVec3::Z),
            shift_held: false,
            ctrl_held: false,
            alt_held: false,
        };
        line_tool.on_click(&mut scene, &ctx0);

        let ctx1 = ToolContext {
            screen: None,
            world: DVec3::new(5.0, 0.0, 0.0),
            ray: Ray::new(DVec3::new(5.0, 0.0, 10.0), -DVec3::Z),
            shift_held: false,
            ctrl_held: false,
            alt_held: false,
        };
        line_tool.on_click(&mut scene, &ctx1);

        assert_eq!(scene.mesh.vertices.len(), 2);
        assert_eq!(scene.mesh.edges.len(), 1);
    }

    #[test]
    fn test_rectangle_tool_drawing() {
        let mut scene = Scene::new();
        let mut rect_tool = rectangle::RectangleTool::default();

        let ctx0 = ToolContext {
            screen: None,
            world: DVec3::new(0.0, 0.0, 0.0),
            ray: Ray::new(DVec3::new(0.0, 0.0, 10.0), -DVec3::Z),
            shift_held: false,
            ctrl_held: false,
            alt_held: false,
        };
        rect_tool.on_click(&mut scene, &ctx0);

        let ctx1 = ToolContext {
            screen: None,
            world: DVec3::new(4.0, 3.0, 0.0),
            ray: Ray::new(DVec3::new(4.0, 3.0, 10.0), -DVec3::Z),
            shift_held: false,
            ctrl_held: false,
            alt_held: false,
        };
        rect_tool.on_click(&mut scene, &ctx1);

        assert_eq!(scene.mesh.vertices.len(), 4);
        assert_eq!(scene.mesh.faces.len(), 1);
    }

    #[test]
    fn test_polygon_tool() {
        let mut scene = Scene::new();
        let mut poly_tool = polygon::PolygonTool::default();
        poly_tool.sides = 6;

        let ctx0 = ToolContext::new(DVec3::ZERO, Ray::new(DVec3::new(0.0, 0.0, 10.0), -DVec3::Z));
        poly_tool.on_click(&mut scene, &ctx0);

        let ctx1 = ToolContext::new(DVec3::new(2.0, 0.0, 0.0), Ray::new(DVec3::new(2.0, 0.0, 10.0), -DVec3::Z));
        poly_tool.on_click(&mut scene, &ctx1);

        assert_eq!(scene.mesh.vertices.len(), 6);
        assert_eq!(scene.mesh.faces.len(), 1);
    }

    #[test]
    fn test_dimension_tool() {
        let mut scene = Scene::new();
        let mut dim_tool = dimension::DimensionTool::default();

        let ctx0 = ToolContext::new(DVec3::ZERO, Ray::new(DVec3::new(0.0, 0.0, 10.0), -DVec3::Z));
        let ctx1 = ToolContext::new(DVec3::new(10.0, 0.0, 0.0), Ray::new(DVec3::new(10.0, 0.0, 10.0), -DVec3::Z));
        let ctx2 = ToolContext::new(DVec3::new(5.0, 2.0, 0.0), Ray::new(DVec3::new(5.0, 2.0, 10.0), -DVec3::Z));

        dim_tool.on_click(&mut scene, &ctx0);
        dim_tool.on_click(&mut scene, &ctx1);
        dim_tool.on_click(&mut scene, &ctx2);

        assert_eq!(scene.dimensions.len(), 1);
        assert!((scene.dimensions[0].length() - 10.0).abs() < 1e-6);
    }

    #[test]
    fn test_section_plane_tool() {
        let mut scene = Scene::new();
        let mut sec_tool = section::SectionPlaneTool::default();

        let ctx = ToolContext::new(DVec3::new(0.0, 0.0, 2.5), Ray::new(DVec3::new(0.0, 0.0, 10.0), -DVec3::Z));
        sec_tool.on_click(&mut scene, &ctx);

        assert_eq!(scene.section_planes.len(), 1);
        assert_eq!(scene.section_planes[0].symbol, "A");
        assert!(scene.show_section_cuts);
    }
}
