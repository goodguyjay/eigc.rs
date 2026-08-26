use super::HeightSource;
use bevy::prelude::Vec2;
use noise::{NoiseFn, Perlin};

pub struct Warp2D<S: HeightSource> {
    pub source: S,
    pub perlin: Perlin,
    pub warp_amp: f32, // meters of warp displacement
    pub warp_freq: f32,
    pub octaves: u32,
    pub lacunarity: f32,
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

/// Projects coordinates onto an oriented axis to create anisotropy
pub struct Oriented<S: HeightSource> {
    pub source: S,
    pub dir: Vec2, // must be normalized
    /// scale along the main axis (dir) and orthogonal axis
    pub main_scale: f32,
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