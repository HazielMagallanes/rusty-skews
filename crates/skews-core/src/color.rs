//! RGBA color type and hex parsing.

use std::error::Error;
use std::fmt;

/// A color with straight alpha, stored as `f32` components in `0.0..=1.0`.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Rgba {
    /// Red channel.
    pub r: f32,
    /// Green channel.
    pub g: f32,
    /// Blue channel.
    pub b: f32,
    /// Alpha channel.
    pub a: f32,
}

impl Default for Rgba {
    /// Transparent black, the neutral default for color fields.
    fn default() -> Self {
        Self::TRANSPARENT
    }
}

impl Rgba {
    /// Fully transparent black.
    pub const TRANSPARENT: Self = Self {
        r: 0.0,
        g: 0.0,
        b: 0.0,
        a: 0.0,
    };

    /// Opaque white.
    pub const WHITE: Self = Self {
        r: 1.0,
        g: 1.0,
        b: 1.0,
        a: 1.0,
    };

    /// Creates a color from components (not clamped).
    #[must_use]
    pub const fn new(r: f32, g: f32, b: f32, a: f32) -> Self {
        Self { r, g, b, a }
    }

    /// Creates an opaque color from RGB components.
    #[must_use]
    pub const fn rgb(r: f32, g: f32, b: f32) -> Self {
        Self { r, g, b, a: 1.0 }
    }

    /// Parses `#rrggbb` or `#rrggbbaa` (the leading `#` is optional).
    pub fn from_hex(hex: &str) -> Result<Self, ColorError> {
        let value = hex.trim().trim_start_matches('#');

        let parse = |slice: &str| u8::from_str_radix(slice, 16);
        let (r, g, b, a) = match value.len() {
            6 => {
                let r = parse(&value[0..2]).map_err(|_| ColorError::invalid(hex))?;
                let g = parse(&value[2..4]).map_err(|_| ColorError::invalid(hex))?;
                let b = parse(&value[4..6]).map_err(|_| ColorError::invalid(hex))?;
                (r, g, b, 255)
            }
            8 => {
                let r = parse(&value[0..2]).map_err(|_| ColorError::invalid(hex))?;
                let g = parse(&value[2..4]).map_err(|_| ColorError::invalid(hex))?;
                let b = parse(&value[4..6]).map_err(|_| ColorError::invalid(hex))?;
                let a = parse(&value[6..8]).map_err(|_| ColorError::invalid(hex))?;
                (r, g, b, a)
            }
            _ => return Err(ColorError::invalid(hex)),
        };

        Ok(Self::new(
            f32::from(r) / 255.0,
            f32::from(g) / 255.0,
            f32::from(b) / 255.0,
            f32::from(a) / 255.0,
        ))
    }

    /// Returns the components as an array, ready for GPU uniforms.
    #[must_use]
    pub const fn to_array(self) -> [f32; 4] {
        [self.r, self.g, self.b, self.a]
    }

    /// Returns a copy with a different alpha channel.
    #[must_use]
    pub const fn with_alpha(self, alpha: f32) -> Self {
        Self { a: alpha, ..self }
    }
}

/// Error returned when a hex color cannot be parsed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ColorError {
    input: String,
}

impl ColorError {
    fn invalid(input: &str) -> Self {
        Self {
            input: input.to_owned(),
        }
    }

    /// Returns the offending input.
    #[must_use]
    pub fn input(&self) -> &str {
        &self.input
    }
}

impl fmt::Display for ColorError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "invalid hex color: {:?} (expected #rrggbb or #rrggbbaa)",
            self.input
        )
    }
}

impl Error for ColorError {}

#[cfg(test)]
mod tests {
    use super::{ColorError, Rgba};

    #[test]
    fn parses_six_digit_hex() {
        let color = Rgba::from_hex("#ff8800").unwrap();

        assert!((color.r - 1.0).abs() < f32::EPSILON);
        assert!((color.g - 0.533_333_3).abs() < 1e-6);
        assert!((color.b - 0.0).abs() < f32::EPSILON);
        assert!((color.a - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn parses_eight_digit_hex_and_optional_hash() {
        let color = Rgba::from_hex("ff880080").unwrap();

        assert!((color.a - 128.0 / 255.0).abs() < 1e-6);
    }

    #[test]
    fn rejects_invalid_input() {
        for input in ["", "#xyzxyz", "#ff88", "#ff8800ff00", "ff88"] {
            let err = Rgba::from_hex(input).unwrap_err();
            assert_eq!(err, ColorError::invalid(input));
            assert_eq!(err.input(), input);
        }
    }

    #[test]
    fn with_alpha_keeps_channels() {
        let color = Rgba::from_hex("#112233").unwrap().with_alpha(0.5);

        assert_eq!(
            color.to_array()[0..3],
            [
                0x11 as f32 / 255.0,
                0x22 as f32 / 255.0,
                0x33 as f32 / 255.0
            ]
        );
        assert!((color.a - 0.5).abs() < f32::EPSILON);
    }
}
