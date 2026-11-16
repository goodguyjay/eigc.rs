//! Manchas de cor por máscara de ruído, por cima de outra fonte de cor.

use super::{ColorSource, HeightSource};
use eigc_common::math::{lerp4, smoothstep};

/// Pinta `patch_color` por cima de `base` onde a máscara passa do limiar, com borda suave entre
/// `mask_start` e `mask_end`. A máscara é uma `HeightSource` usada só como campo escalar (ex.:
/// `PerlinFbm`), não como altura do terreno.
pub struct ColorPatches<C: ColorSource, M: HeightSource> {
    /// Cor de fundo.
    pub base: C,
    /// Campo escalar que decide onde ficam as manchas.
    pub mask: M,
    /// Cor (RGBA linear) das manchas.
    pub patch_color: [f32; 4],
    /// Valor da máscara abaixo do qual fica só a cor de fundo.
    pub mask_start: f32,
    /// Valor da máscara a partir do qual fica só a cor da mancha.
    pub mask_end: f32,
}

impl<C: ColorSource, M: HeightSource> ColorSource for ColorPatches<C, M> {
    fn color_at(&self, x: f32, z: f32) -> [f32; 4] {
        let weight = smoothstep(self.mask_start, self.mask_end, self.mask.height_at(x, z));
        lerp4(self.base.color_at(x, z), self.patch_color, weight)
    }
}
