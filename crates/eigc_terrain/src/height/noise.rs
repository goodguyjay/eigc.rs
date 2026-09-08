//! Fontes de altura baseadas em ruído Perlin (fbm e ridged).

use super::HeightSource;
use noise::{NoiseFn, Perlin};

/// Fonte de altura por ruído Perlin fractal (fbm - soma de oitavas com amplitude decrescente).
pub struct PerlinFbm {
    /// Gerador de ruído Perlin subjacente.
    pub perlin: Perlin,
    /// Frequência base do ruído (menor → características mais amplas).
    pub freq: f32,
    /// Número de oitavas somadas.
    pub octaves: u32,
    /// Multiplicador de frequência aplicado a cada oitava sucessiva.
    pub lacunarity: f32,
    /// Multiplicador de amplitude aplicado a cada oitava sucessiva.
    pub gain: f32,
    /// Amplitude final aplicada ao resultado normalizado.
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

/// Fonte de altura por ruído Perlin "ridged" (cristas), útil para relevo acidentado.
pub struct PerlinRidged {
    /// Gerador de ruído Perlin subjacente.
    pub perlin: Perlin,
    /// Frequência base do ruído (menor → características mais amplas).
    pub freq: f32,
    /// Número de oitavas somadas.
    pub octaves: u32,
    /// Multiplicador de frequência aplicado a cada oitava sucessiva.
    pub lacunarity: f32,
    /// Multiplicador de amplitude aplicado a cada oitava sucessiva.
    pub gain: f32,
    /// Amplitude final aplicada ao resultado normalizado.
    pub amplitude: f32,
    /// Fator de anisotropia aplicado ao eixo z entre oitavas.
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
