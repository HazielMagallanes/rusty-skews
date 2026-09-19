//! Pixel geometry primitives used for layout and damage tracking.

/// A width/height pair in pixels.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub struct Size {
    /// Horizontal extent in pixels.
    pub width: u32,
    /// Vertical extent in pixels.
    pub height: u32,
}

impl Size {
    /// The zero size.
    pub const ZERO: Self = Self {
        width: 0,
        height: 0,
    };

    /// Creates a size from width and height.
    #[must_use]
    pub const fn new(width: u32, height: u32) -> Self {
        Self { width, height }
    }

    /// Returns `true` when either dimension is zero.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.width == 0 || self.height == 0
    }
}

/// A rectangle in pixel coordinates with a top-left origin.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub struct Rect {
    /// Horizontal offset of the top-left corner.
    pub x: i32,
    /// Vertical offset of the top-left corner.
    pub y: i32,
    /// Horizontal extent in pixels.
    pub width: u32,
    /// Vertical extent in pixels.
    pub height: u32,
}

impl Rect {
    /// The zero rectangle at the origin.
    pub const ZERO: Self = Self {
        x: 0,
        y: 0,
        width: 0,
        height: 0,
    };

    /// Creates a rectangle from position and size.
    #[must_use]
    pub const fn new(x: i32, y: i32, width: u32, height: u32) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    /// Creates a rectangle from position and size.
    #[must_use]
    pub const fn from_size(x: i32, y: i32, size: Size) -> Self {
        Self {
            x,
            y,
            width: size.width,
            height: size.height,
        }
    }

    /// Returns `true` when either dimension is zero.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.width == 0 || self.height == 0
    }

    /// Exclusive right edge (`x + width`), saturating on overflow.
    #[must_use]
    pub fn right(&self) -> i32 {
        let right = i64::from(self.x) + i64::from(self.width);
        right.clamp(i64::from(i32::MIN), i64::from(i32::MAX)) as i32
    }

    /// Exclusive bottom edge (`y + height`), saturating on overflow.
    #[must_use]
    pub fn bottom(&self) -> i32 {
        let bottom = i64::from(self.y) + i64::from(self.height);
        bottom.clamp(i64::from(i32::MIN), i64::from(i32::MAX)) as i32
    }

    /// Returns `true` when the point lies inside the half-open rectangle.
    #[must_use]
    pub fn contains(&self, px: i32, py: i32) -> bool {
        let (px, py) = (i64::from(px), i64::from(py));
        px >= i64::from(self.x)
            && py >= i64::from(self.y)
            && px < i64::from(self.x) + i64::from(self.width)
            && py < i64::from(self.y) + i64::from(self.height)
    }

    /// Returns the overlapping rectangle with `other`, if any.
    ///
    /// Touching edges are not an intersection (half-open semantics).
    #[must_use]
    pub fn intersection(&self, other: &Self) -> Option<Self> {
        let x1 = i64::from(self.x).max(i64::from(other.x));
        let y1 = i64::from(self.y).max(i64::from(other.y));
        let x2 = (i64::from(self.x) + i64::from(self.width))
            .min(i64::from(other.x) + i64::from(other.width));
        let y2 = (i64::from(self.y) + i64::from(self.height))
            .min(i64::from(other.y) + i64::from(other.height));

        if x2 <= x1 || y2 <= y1 {
            return None;
        }

        Some(Self {
            x: x1 as i32,
            y: y1 as i32,
            width: (x2 - x1) as u32,
            height: (y2 - y1) as u32,
        })
    }

    /// Returns the smallest rectangle containing both inputs.
    #[must_use]
    pub fn union(&self, other: &Self) -> Self {
        if self.is_empty() {
            return *other;
        }
        if other.is_empty() {
            return *self;
        }

        let x1 = i64::from(self.x).min(i64::from(other.x));
        let y1 = i64::from(self.y).min(i64::from(other.y));
        let x2 = (i64::from(self.x) + i64::from(self.width))
            .max(i64::from(other.x) + i64::from(other.width));
        let y2 = (i64::from(self.y) + i64::from(self.height))
            .max(i64::from(other.y) + i64::from(other.height));

        Self {
            x: x1 as i32,
            y: y1 as i32,
            width: (x2 - x1) as u32,
            height: (y2 - y1) as u32,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Rect, Size};

    #[test]
    fn size_reports_emptiness() {
        assert!(Size::ZERO.is_empty());
        assert!(Size::new(0, 10).is_empty());
        assert!(Size::new(10, 0).is_empty());
        assert!(!Size::new(10, 10).is_empty());
    }

    #[test]
    fn edges_and_containment_use_half_open_ranges() {
        let rect = Rect::new(10, 20, 100, 50);

        assert_eq!(rect.right(), 110);
        assert_eq!(rect.bottom(), 70);
        assert!(rect.contains(10, 20));
        assert!(rect.contains(109, 69));
        assert!(!rect.contains(110, 69));
        assert!(!rect.contains(109, 70));
        assert!(!rect.contains(9, 20));
    }

    #[test]
    fn intersection_handles_disjoint_touching_and_contained() {
        let rect = Rect::new(0, 0, 10, 10);

        assert_eq!(rect.intersection(&Rect::new(20, 20, 5, 5)), None);
        assert_eq!(rect.intersection(&Rect::new(10, 0, 5, 5)), None);
        assert_eq!(
            rect.intersection(&Rect::new(5, 5, 10, 10)),
            Some(Rect::new(5, 5, 5, 5))
        );
        assert_eq!(
            rect.intersection(&Rect::new(2, 2, 3, 3)),
            Some(Rect::new(2, 2, 3, 3))
        );
    }

    #[test]
    fn union_ignores_empty_rectangles() {
        let rect = Rect::new(0, 0, 10, 10);

        assert_eq!(rect.union(&Rect::ZERO), rect);
        assert_eq!(Rect::ZERO.union(&rect), rect);
        assert_eq!(
            rect.union(&Rect::new(5, 5, 10, 10)),
            Rect::new(0, 0, 15, 15)
        );
    }
}
