//! Cor por vértice derivada da inclinação de uma fonte de altura.

use super::{ColorSource, HeightSource};
use bevy::prelude::Vec2;
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

/// Gradiente da fonte no ponto (x, z), por diferença central com passo `step` (subida por
/// avanço em X e em Z).
fn gradient_at<S: HeightSource>(source: &S, step: f32, x: f32, z: f32) -> Vec2 {
    Vec2::new(
        (source.height_at(x + step, z) - source.height_at(x - step, z)) / (2.0 * step),
        (source.height_at(x, z + step) - source.height_at(x, z - step)) / (2.0 * step),
    )
}

impl<S: HeightSource> SlopeColorField<S> {
    /// Inclinação (módulo do gradiente, subida/avanço) da fonte no ponto (x, z).
    fn slope_at(&self, x: f32, z: f32) -> f32 {
        gradient_at(&self.source, self.sample_step_m, x, z).length()
    }
}

impl<S: HeightSource> ColorSource for SlopeColorField<S> {
    fn color_at(&self, x: f32, z: f32) -> [f32; 4] {
        let t = smoothstep(self.slope_start, self.slope_end, self.slope_at(x, z));
        lerp4(self.flat_color, self.steep_color, t)
    }
}

/// Pinta geada nas encostas voltadas para o polo, por cima de uma fonte de cor. Em latitude alta,
/// essas encostas recebem menos sol e ficam frias o bastante para guardar geada, como na borda
/// das calotas polares de Ganimedes. Uma máscara de ruído quebra a geada em manchas.
pub struct PoleFacingFrost<C: ColorSource, S: HeightSource, M: HeightSource> {
    /// Cor de fundo.
    pub base: C,
    /// Fonte de altura cuja orientação decide onde há geada.
    pub source: S,
    /// Distância, em metros, entre as amostras do gradiente.
    pub sample_step_m: f32,
    /// Direção horizontal para o polo, no plano XZ (unitária).
    pub pole: Vec2,
    /// Cor (RGBA linear) da geada.
    pub frost_color: [f32; 4],
    /// Inclinação voltada para o polo (subida/avanço) em que a geada começa.
    pub facing_start: f32,
    /// Inclinação voltada para o polo a partir da qual a geada é plena (antes da máscara).
    pub facing_end: f32,
    /// Campo escalar que quebra a geada em manchas.
    pub mask: M,
    /// Valor da máscara abaixo do qual não há geada.
    pub mask_start: f32,
    /// Valor da máscara a partir do qual a geada não é atenuada.
    pub mask_end: f32,
}

impl<C: ColorSource, S: HeightSource, M: HeightSource> PoleFacingFrost<C, S, M> {
    /// Quanto a encosta está voltada para o polo: a inclinação na direção oposta ao polo (a
    /// encosta sobe se afastando dele, então a face olha para ele). Negativa nas encostas
    /// voltadas para o equador.
    pub fn pole_facing_slope_at(&self, x: f32, z: f32) -> f32 {
        -gradient_at(&self.source, self.sample_step_m, x, z).dot(self.pole.normalize())
    }
}

impl<C: ColorSource, S: HeightSource, M: HeightSource> ColorSource for PoleFacingFrost<C, S, M> {
    fn color_at(&self, x: f32, z: f32) -> [f32; 4] {
        let facing = smoothstep(
            self.facing_start,
            self.facing_end,
            self.pole_facing_slope_at(x, z),
        );
        let patches = smoothstep(self.mask_start, self.mask_end, self.mask.height_at(x, z));
        lerp4(self.base.color_at(x, z), self.frost_color, facing * patches)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Rampa que sobe para o sul (+Z): a face olha para o norte (-Z).
    struct RisingSouth;
    impl HeightSource for RisingSouth {
        fn height_at(&self, _x: f32, z: f32) -> f32 {
            0.3 * z
        }
    }

    struct One;
    impl HeightSource for One {
        fn height_at(&self, _x: f32, _z: f32) -> f32 {
            1.0
        }
    }

    struct Black;
    impl ColorSource for Black {
        fn color_at(&self, _x: f32, _z: f32) -> [f32; 4] {
            [0.0, 0.0, 0.0, 1.0]
        }
    }

    fn frost(pole: Vec2) -> PoleFacingFrost<Black, RisingSouth, One> {
        PoleFacingFrost {
            base: Black,
            source: RisingSouth,
            sample_step_m: 10.0,
            pole,
            frost_color: [1.0, 1.0, 1.0, 1.0],
            facing_start: 0.05,
            facing_end: 0.2,
            mask: One,
            mask_start: 0.0,
            mask_end: 0.5,
        }
    }

    /// Testa que a encosta voltada para o polo (norte, -Z) recebe geada e a mesma encosta, com o
    /// polo do outro lado, não recebe.
    #[test]
    fn frost_covers_only_pole_facing_slopes() {
        let north = frost(Vec2::new(0.0, -1.0));
        let south = frost(Vec2::new(0.0, 1.0));

        assert!((north.pole_facing_slope_at(0.0, 0.0) - 0.3).abs() < 1e-4);
        assert_eq!(north.color_at(0.0, 0.0)[0], 1.0);
        assert_eq!(south.color_at(0.0, 0.0)[0], 0.0);
    }
}
