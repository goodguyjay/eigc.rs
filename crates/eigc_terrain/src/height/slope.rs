//! Cor por vértice derivada da inclinação de uma fonte de altura.

use super::{ColorSource, HeightSource};
use eigc_common::math::{lerp4, smoothstep};

/// Pinta o terreno conforme a inclinação local de uma fonte de altura: `flat_color` em terreno
/// plano, transicionando suavemente para `steep_color` nas encostas íngremes. Em Europa isso
/// destaca os flancos das cristas reais do DTM sem precisar de uma curva de linea artificial.
pub struct SlopeColorField<S: HeightSource> {
    /// Fonte de altura cuja inclinação define a cor.
    pub source: S,
    /// Distância, em metros, entre as amostras da diferença central usada para estimar o gradiente.
    pub sample_step_m: f32,
    /// Cor (RGBA linear) em terreno plano.
    pub flat_color: [f32; 4],
    /// Cor (RGBA linear) em encostas íngremes.
    pub steep_color: [f32; 4],
    /// Inclinação (subida/avanço, adimensional) abaixo da qual a cor é totalmente `flat_color`.
    pub slope_start: f32,
    /// Inclinação (subida/avanço, adimensional) a partir da qual a cor é totalmente `steep_color`.
    pub slope_end: f32,
}

impl<S: HeightSource> SlopeColorField<S> {
    /// Inclinação (módulo do gradiente, subida/avanço) da fonte no ponto (x, z).
    fn slope_at(&self, x: f32, z: f32) -> f32 {
        let step = self.sample_step_m;
        let dx = (self.source.height_at(x + step, z) - self.source.height_at(x - step, z))
            / (2.0 * step);
        let dz = (self.source.height_at(x, z + step) - self.source.height_at(x, z - step))
            / (2.0 * step);
        dx.hypot(dz)
    }
}

impl<S: HeightSource> ColorSource for SlopeColorField<S> {
    fn color_at(&self, x: f32, z: f32) -> [f32; 4] {
        let t = smoothstep(self.slope_start, self.slope_end, self.slope_at(x, z));
        lerp4(self.flat_color, self.steep_color, t)
    }
}
