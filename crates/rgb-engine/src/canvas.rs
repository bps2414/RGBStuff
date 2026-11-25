use rgb_core::{DevicePlacement, LedPoint, Rgb};

#[derive(Debug, Clone)]
pub struct Canvas2D {
    pub width: usize,
    pub height: usize,
    /// Packed RGB888 buffer (`width * height * 3` bytes).
    pub pixels: Vec<u8>,
}

impl Default for Canvas2D {
    fn default() -> Self {
        Self::new(Self::DEFAULT_WIDTH, Self::DEFAULT_HEIGHT)
    }
}

impl Canvas2D {
    pub const DEFAULT_WIDTH: usize = 128;
    pub const DEFAULT_HEIGHT: usize = 72;

    pub fn new(width: usize, height: usize) -> Self {
        let width = width.max(1);
        let height = height.max(1);
        Self {
            width,
            height,
            pixels: vec![0; width * height * 3],
        }
    }

    #[inline]
    pub fn fill(&mut self, color: Rgb) {
        for chunk in self.pixels.chunks_exact_mut(3) {
            chunk[0] = color.r;
            chunk[1] = color.g;
            chunk[2] = color.b;
        }
    }

    #[inline]
    pub fn set_pixel(&mut self, x: usize, y: usize, color: Rgb) {
        if x < self.width && y < self.height {
            let idx = (y * self.width + x) * 3;
            self.pixels[idx] = color.r;
            self.pixels[idx + 1] = color.g;
            self.pixels[idx + 2] = color.b;
        }
    }

    #[inline]
    pub fn get_pixel(&self, x: usize, y: usize) -> Rgb {
        let x = x.min(self.width - 1);
        let y = y.min(self.height - 1);
        let idx = (y * self.width + x) * 3;
        Rgb {
            r: self.pixels[idx],
            g: self.pixels[idx + 1],
            b: self.pixels[idx + 2],
        }
    }

    /// Bilinearly samples a normalized coordinate (`0.0..=1.0`, `0.0..=1.0`).
    pub fn sample_bilinear(&self, nx: f32, ny: f32) -> Rgb {
        let fx = nx.clamp(0.0, 1.0) * (self.width - 1) as f32;
        let fy = ny.clamp(0.0, 1.0) * (self.height - 1) as f32;

        let x0 = fx.floor() as usize;
        let y0 = fy.floor() as usize;
        let x1 = (x0 + 1).min(self.width - 1);
        let y1 = (y0 + 1).min(self.height - 1);

        let tx = fx - x0 as f32;
        let ty = fy - y0 as f32;

        let c00 = self.get_pixel(x0, y0);
        let c10 = self.get_pixel(x1, y0);
        let c01 = self.get_pixel(x0, y1);
        let c11 = self.get_pixel(x1, y1);

        c00.lerp(c10, tx).lerp(c01.lerp(c11, tx), ty)
    }

    pub fn sample_device(
        &self,
        placement: &DevicePlacement,
        local_points: &[LedPoint],
        gain: f32,
    ) -> Vec<Rgb> {
        placement
            .sample_points(local_points)
            .into_iter()
            .map(|pt| self.sample_bilinear(pt.x, pt.y).apply_gain(gain))
            .collect()
    }
}
