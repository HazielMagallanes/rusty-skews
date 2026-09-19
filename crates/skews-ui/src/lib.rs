//! UI scene construction: the draw lists consumed by the renderer.
//!
//! Scenes are plain data (ADR-0002), which keeps UI logic unit-testable and
//! snapshot-friendly without a GPU.

#![forbid(unsafe_code)]

use skews_core::{Rect, Rgba};

/// A single draw item.
#[derive(Debug, Clone, PartialEq)]
pub enum DrawItem {
    /// A filled rounded rectangle.
    Rect {
        /// Rectangle in physical pixels.
        rect: Rect,
        /// Fill color (straight alpha).
        color: Rgba,
        /// Corner radius in pixels.
        radius: f32,
    },
    /// A text run positioned at its top-left corner.
    Text {
        /// Text content.
        text: String,
        /// Left edge in physical pixels.
        x: i32,
        /// Top edge in physical pixels.
        y: i32,
        /// Font size in pixels.
        size: f32,
        /// Text color (straight alpha).
        color: Rgba,
    },
}

/// A frame's worth of drawing.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Scene {
    /// Background color filling the whole surface.
    pub background: Rgba,
    /// Draw items, in paint order.
    pub items: Vec<DrawItem>,
}

/// One text item to place in the bar.
#[derive(Debug, Clone, PartialEq)]
pub struct BarText {
    /// Text content.
    pub text: String,
    /// Left edge in pixels (may be fractional; rounded when placed).
    pub x: f32,
    /// Top edge in pixels (may be fractional; rounded when placed).
    pub y: f32,
    /// Font size in pixels.
    pub size: f32,
    /// Text color.
    pub color: Rgba,
}

/// Builds a bar scene: a solid background plus one text item per module.
#[must_use]
pub fn build_bar_scene(background: Rgba, texts: &[BarText]) -> Scene {
    Scene {
        background,
        items: texts
            .iter()
            .map(|item| DrawItem::Text {
                text: item.text.clone(),
                x: item.x.round() as i32,
                y: item.y.round() as i32,
                size: item.size,
                color: item.color,
            })
            .collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::{BarText, DrawItem, build_bar_scene};
    use skews_core::Rgba;

    #[test]
    fn bar_scene_rounds_positions_and_keeps_order() {
        let background = Rgba::from_hex("#1e1e2e").unwrap();
        let scene = build_bar_scene(
            background,
            &[
                BarText {
                    text: String::from("workspaces"),
                    x: 8.4,
                    y: 9.6,
                    size: 12.0,
                    color: Rgba::WHITE,
                },
                BarText {
                    text: String::from("18:33"),
                    x: 920.0,
                    y: 9.0,
                    size: 12.0,
                    color: Rgba::WHITE,
                },
            ],
        );

        assert_eq!(scene.background, background);
        assert_eq!(scene.items.len(), 2);
        match &scene.items[0] {
            DrawItem::Text { x, y, text, .. } => {
                assert_eq!((*x, *y), (8, 10));
                assert_eq!(text, "workspaces");
            }
            other => panic!("expected text, got {other:?}"),
        }
    }

    #[test]
    fn empty_scene_is_valid() {
        let scene = build_bar_scene(Rgba::TRANSPARENT, &[]);

        assert!(scene.items.is_empty());
    }
}
