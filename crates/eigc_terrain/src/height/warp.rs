//! Distorção de domínio e projeção anisotrópica de `HeightSource`.

use super::HeightSource;
use bevy::prelude::Vec2;
use noise::{NoiseFn, Perlin};

/// Distorce as coordenadas (x, z) com um campo de ruído fbm antes de consultar a fonte de altura.
pub struct Warp2D<S: HeightSource> {
    /// Fonte de altura consultada com as coordenadas já distorcidas.
    pub source: S,
    /// Gerador de ruído Perlin usado para o campo de distorção.
    pub perlin: Perlin,
    /// Amplitude do deslocamento de distorção, em metros.
    pub warp_amp: f32,
    /// Frequência base do campo de distorção.
    pub warp_freq: f32,
    /// Número de oitavas somadas no campo de distorção.
    pub octaves: u32,
    /// Multiplicador de frequência aplicado a cada oitava sucessiva.
    pub lacunarity: f32,
    /// Multiplicador de amplitude aplicado a cada oitava sucessiva.
    pub gain: f32,
}

impl<S: HeightSource> HeightSource for Warp2D<S> {
    fn height_at(&self, x: f32, z: f32) -> f32 {
        // small fbm field for displacement
        let mut a = 1.0;
        let mut sumx = 0.0;
        let mut sumz = 0.0;
        let mut amp = 0.0;
        
        let mut fx = x * self.warp_freq;
        let mut fz = z * self.warp_freq;
        
        for _ in 0..self.octaves {
            // two independent samples for x/z displacement
            let nx = self.perlin.get([fx as f64, fz as f64]) as f32;
            let nz = self.perlin.get([fz as f64, fx as f64]) as f32;
            
            sumx += a * nx;
            sumz += a * nz;
            amp += a;
            
            fx *= self.lacunarity;
            fz *= self .lacunarity;
            a *= self.gain;
        }
        
        let wx = (sumx / amp) * self.warp_amp;
        let wz = (sumz / amp) * self.warp_amp;
        
        self.source.height_at(x + wx, z + wz)
    }
}

/// Projeta as coordenadas sobre um eixo orientado para criar anisotropia.
pub struct Oriented<S: HeightSource> {
    /// Fonte de altura consultada com as coordenadas já projetadas.
    pub source: S,
    /// Direção do eixo principal de projeção (deve ser normalizada).
    pub dir: Vec2,
    /// Escala aplicada ao longo do eixo principal (`dir`).
    pub main_scale: f32,
    /// Escala aplicada ao longo do eixo ortogonal ao principal.
    pub ortho_scale: f32,
}

impl<S: HeightSource> HeightSource for Oriented<S> {
    fn height_at(&self, x: f32, z: f32) -> f32 {
        let p = Vec2::new(x, z);
        let t = self.dir;
        let n = Vec2::new(-t.y, t.x);
        let u = p.dot(t) * self.main_scale;
        let v = p.dot(n) * self.ortho_scale;
        self.source.height_at(u, v)
    }
}