use super::HeightSource;
use noise::{NoiseFn, Perlin};

pub struct PerlinFbm {
    pub perlin: Perlin,
    pub freq: f32,
    pub octaves: u32,
    pub lacunarity: f32,
    pub gain: f32,
    pub amplitude: f32,
}

impl HeightSource for PerlinFbm {
    fn height_at(&self, x: f32, z: f32) -> f32 {
        let mut a = 1.0;
        let mut sum = 0.0;
        let mut amp = 0.0;
        let mut fx = x * self.freq;
        let mut fz = z * self.freq;

        for _ in 0..self.octaves {
            sum += a * self.perlin.get([fx as f64, fz as f64]) as f32;
            amp += a;
            fx *= self.lacunarity;
            fz *= self.lacunarity;
            a *= self.gain;
        }

        (sum / amp) * self.amplitude
    }
}

pub struct PerlinRidged {
    pub perlin: Perlin,
    pub freq: f32,
    pub octaves: u32,
    pub lacunarity: f32,
    pub gain: f32,
    pub amplitude: f32,
    pub z_anisotropy: f32,
}

impl HeightSource for PerlinRidged {
    fn height_at(&self, x: f32, z: f32) -> f32 {
        let mut a = 1.0;
        let mut sum = 0.0;
        let mut amp = 0.0;
        let mut fx = x * self.freq;
        let mut fz = z * self.freq;

        for _ in 0..self.octaves {
            let v = 1.0 - (self.perlin.get([fx as f64, fz as f64]) as f32).abs();
            sum += a * (v * v);
            amp += a;
            fx *= self.lacunarity;
            fz *= self.lacunarity * self.z_anisotropy;
            a *= self.gain;
        }

        (sum / amp).clamp(0.0, 1.0) * self.amplitude
    }
}
