//! Bar layout built on `taffy`.
//!
//! The bar is a single flex row with three equal-width regions (left, center,
//! right); text items are measured through taffy's measure callback, so the
//! layout engine needs sizes only — never fonts or a GPU.

#![forbid(unsafe_code)]

use taffy::prelude::*;
use taffy::{LayoutInput, LayoutOutput, Point, TaffyError};

/// Measured size of a text run.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TextMetrics {
    /// Advance width in pixels.
    pub width: f32,
    /// Line height in pixels.
    pub height: f32,
}

/// A text position (top-left corner) inside the bar.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PlacedText {
    /// Horizontal offset in pixels.
    pub x: f32,
    /// Vertical offset in pixels.
    pub y: f32,
}

/// Positions for every text item of the bar, per region.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct BarLayout {
    /// Left region positions, in configuration order.
    pub left: Vec<PlacedText>,
    /// Center region positions, in configuration order.
    pub center: Vec<PlacedText>,
    /// Right region positions, in configuration order.
    pub right: Vec<PlacedText>,
}

/// Inputs for one layout pass.
#[derive(Debug, Clone, Copy)]
pub struct BarLayoutOptions {
    /// Bar width in pixels.
    pub width: f32,
    /// Bar height in pixels.
    pub height: f32,
    /// Horizontal padding at both ends in pixels.
    pub padding: f32,
    /// Gap between items of the same region in pixels.
    pub gap: f32,
}

impl BarLayoutOptions {
    /// Options with the shell's default padding (8 px) and gap (10 px).
    #[must_use]
    pub const fn new(width: f32, height: f32) -> Self {
        Self {
            width,
            height,
            padding: 8.0,
            gap: 10.0,
        }
    }
}

/// Computes positions for every text item of the bar.
pub fn layout_bar(
    options: BarLayoutOptions,
    left: &[TextMetrics],
    center: &[TextMetrics],
    right: &[TextMetrics],
) -> Result<BarLayout, TaffyError> {
    let mut tree: TaffyTree<TextMetrics> = TaffyTree::new();

    let left_leaves = add_leaves(&mut tree, left)?;
    let center_leaves = add_leaves(&mut tree, center)?;
    let right_leaves = add_leaves(&mut tree, right)?;

    let left_region = tree.new_with_children(
        region_style(JustifyContent::START, options.gap),
        &left_leaves,
    )?;
    let center_region = tree.new_with_children(
        region_style(JustifyContent::CENTER, options.gap),
        &center_leaves,
    )?;
    let right_region = tree.new_with_children(
        region_style(JustifyContent::END, options.gap),
        &right_leaves,
    )?;

    let root = tree.new_with_children(
        Style {
            display: Display::Flex,
            flex_direction: FlexDirection::Row,
            align_items: Some(AlignItems::CENTER),
            size: Size {
                width: Dimension::from_length(options.width),
                height: Dimension::from_length(options.height),
            },
            padding: Rect {
                left: LengthPercentage::from_length(options.padding),
                right: LengthPercentage::from_length(options.padding),
                top: LengthPercentage::from_length(options.padding),
                bottom: LengthPercentage::from_length(options.padding),
            },
            ..Default::default()
        },
        &[left_region, center_region, right_region],
    )?;

    tree.compute_layout_with_measure(
        root,
        Size {
            width: AvailableSpace::Definite(options.width),
            height: AvailableSpace::Definite(options.height),
        },
        |_input: LayoutInput, _node: NodeId, context: Option<&mut TextMetrics>, _style: &Style| {
            context.map_or(LayoutOutput::DEFAULT, |metrics| {
                LayoutOutput::from_outer_size(Size {
                    width: metrics.width,
                    height: metrics.height,
                })
            })
        },
    )?;

    let root_location = tree.layout(root)?.location;
    Ok(BarLayout {
        left: collect_positions(&tree, root_location, left_region, &left_leaves)?,
        center: collect_positions(&tree, root_location, center_region, &center_leaves)?,
        right: collect_positions(&tree, root_location, right_region, &right_leaves)?,
    })
}

fn add_leaves(
    tree: &mut TaffyTree<TextMetrics>,
    items: &[TextMetrics],
) -> Result<Vec<NodeId>, TaffyError> {
    items
        .iter()
        .map(|metrics| tree.new_leaf_with_context(Style::default(), *metrics))
        .collect()
}

fn region_style(justify: AlignContent, gap: f32) -> Style {
    Style {
        display: Display::Flex,
        flex_direction: FlexDirection::Row,
        align_items: Some(AlignItems::CENTER),
        justify_content: Some(justify),
        gap: Size {
            width: LengthPercentage::from_length(gap),
            height: LengthPercentage::from_length(0.0),
        },
        flex_grow: 1.0,
        flex_basis: Dimension::from_length(0.0),
        ..Default::default()
    }
}

fn collect_positions(
    tree: &TaffyTree<TextMetrics>,
    root_location: Point<f32>,
    region: NodeId,
    leaves: &[NodeId],
) -> Result<Vec<PlacedText>, TaffyError> {
    let region_location = tree.layout(region)?.location;

    leaves
        .iter()
        .map(|leaf| {
            let location = tree.layout(*leaf)?.location;
            Ok(PlacedText {
                x: root_location.x + region_location.x + location.x,
                y: root_location.y + region_location.y + location.y,
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{BarLayoutOptions, TextMetrics, layout_bar};

    fn metrics(width: f32) -> TextMetrics {
        TextMetrics {
            width,
            height: 16.0,
        }
    }

    fn approx(actual: f32, expected: f32) {
        assert!(
            (actual - expected).abs() <= 1.0,
            "expected {expected} ± 1.0, got {actual}"
        );
    }

    #[test]
    fn regions_are_padded_and_aligned() {
        let options = BarLayoutOptions::new(1920.0, 32.0);
        let layout = layout_bar(
            options,
            &[metrics(50.0)],
            &[metrics(80.0)],
            &[metrics(40.0), metrics(60.0)],
        )
        .unwrap();

        // Left starts after the padding.
        approx(layout.left[0].x, 8.0);
        // Center is centered in the bar.
        approx(layout.center[0].x, (1920.0 - 80.0) / 2.0);
        // Right ends before the padding, in order, with the configured gap.
        approx(layout.right[1].x + 60.0, 1920.0 - 8.0);
        approx(layout.right[1].x, layout.right[0].x + 40.0 + options.gap);
    }

    #[test]
    fn items_are_vertically_centered() {
        let options = BarLayoutOptions::new(800.0, 40.0);
        let layout = layout_bar(options, &[metrics(10.0)], &[], &[]).unwrap();

        approx(layout.left[0].y, (40.0 - 16.0) / 2.0);
    }

    #[test]
    fn empty_regions_are_fine() {
        let options = BarLayoutOptions::new(800.0, 32.0);
        let layout = layout_bar(options, &[], &[], &[]).unwrap();

        assert!(layout.left.is_empty() && layout.center.is_empty() && layout.right.is_empty());
    }

    #[test]
    fn many_items_stack_with_gaps() {
        let options = BarLayoutOptions::new(1920.0, 32.0);
        let layout = layout_bar(
            options,
            &[metrics(20.0), metrics(30.0), metrics(40.0)],
            &[],
            &[],
        )
        .unwrap();

        approx(layout.left[1].x, layout.left[0].x + 20.0 + options.gap);
        approx(layout.left[2].x, layout.left[1].x + 30.0 + options.gap);
    }
}
