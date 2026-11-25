use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ColorParseError {
    #[error("invalid hex color length (expected 6 hex chars, got {0})")]
    InvalidLength(usize),
    #[error("invalid hex character in color string")]
    InvalidHex,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Rgb {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl Rgb {
    pub const BLACK: Self = Self { r: 0, g: 0, b: 0 };
    pub const WHITE: Self = Self {
        r: 255,
        g: 255,
        b: 255,
    };

    #[inline]
    pub const fn new(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b }
    }

    pub fn from_hex(input: &str) -> Result<Self, ColorParseError> {
        let hex = input.trim().strip_prefix('#').unwrap_or(input.trim());
        if hex.len() != 6 {
            return Err(ColorParseError::InvalidLength(hex.len()));
        }
        let r = u8::from_str_radix(&hex[0..2], 16).map_err(|_| ColorParseError::InvalidHex)?;
        let g = u8::from_str_radix(&hex[2..4], 16).map_err(|_| ColorParseError::InvalidHex)?;
        let b = u8::from_str_radix(&hex[4..6], 16).map_err(|_| ColorParseError::InvalidHex)?;
        Ok(Self { r, g, b })
    }

    pub fn to_hex(self) -> String {
        format!("#{:02X}{:02X}{:02X}", self.r, self.g, self.b)
    }

    #[inline]
    pub fn lerp(self, other: Self, t: f32) -> Self {
        let t = t.clamp(0.0, 1.0);
        let inv = 1.0 - t;
        Self {
            r: (self.r as f32 * inv + other.r as f32 * t).round() as u8,
            g: (self.g as f32 * inv + other.g as f32 * t).round() as u8,
            b: (self.b as f32 * inv + other.b as f32 * t).round() as u8,
        }
    }

    /// Scales brightness where `brightness` is 0..=100.
    #[inline]
    pub fn scale_brightness(self, brightness: u8) -> Self {
        let b = brightness.min(100) as u16;
        if b == 100 {
            return self;
        }
        if b == 0 {
            return Self::BLACK;
        }
        Self {
            r: ((self.r as u16 * b + 50) / 100) as u8,
            g: ((self.g as u16 * b + 50) / 100) as u8,
            b: ((self.b as u16 * b + 50) / 100) as u8,
        }
    }
}

/// Converts HSV (`h` in `[0, 360)`, `s` in `0..=1`, `v` in `0..=1`) to `Rgb`.
pub fn hsv_to_rgb(h: f32, s: f32, v: f32) -> Rgb {
    let h = h.rem_euclid(360.0);
    let s = s.clamp(0.0, 1.0);
    let v = v.clamp(0.0, 1.0);

    let c = v * s;
    let x = c * (1.0 - ((h / 60.0) % 2.0 - 1.0).abs());
    let m = v - c;

    let (rf, gf, bf) = match h {
        h if h < 60.0 => (c, x, 0.0),
        h if h < 120.0 => (x, c, 0.0),
        h if h < 180.0 => (0.0, c, x),
        h if h < 240.0 => (0.0, x, c),
        h if h < 300.0 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };

    Rgb {
        r: ((rf + m) * 255.0).round() as u8,
        g: ((gf + m) * 255.0).round() as u8,
        b: ((bf + m) * 255.0).round() as u8,
    }
}
