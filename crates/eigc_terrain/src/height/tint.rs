//! Cor por vértice que mistura duas cores por um peso combinado: inclinação, altura relativa ao
//! entorno e manchas de baixa frequência pela posição.

use super::{ColorSource, HeightSource};
use eigc_common::math::{lerp4, smoothstep};

/// Pinta o terreno com `flat_color` e tinge em direção a `tinted_color` onde ele é íngreme ou fica
/// abaixo do entorno, com a intensidade modulada por manchas de baixa frequência.
///
/// O peso de tingimento é `min(1, peso_inclinação + height_weight_scale * peso_altura)` vezes um
/// fator de mancha em `[mottle_min, mottle_max]`, limitado a 1. Há uma única mistura entre as duas
/// cores, então o mais escuro possível é sempre `tinted_color`.
pub struct TerrainTintField<S: HeightSource, B: HeightSource, N: HeightSource> {
    /// Fonte de altura cuja inclinação e altura relativa definem o tingimento.
    pub source: S,
    /// A mesma fonte borrada: a altura relativa é `source - smoothed_source`.
    pub smoothed_source: B,
    /// Ruído de baixa frequência em `[-1, 1]` que define as manchas pela posição.
    pub mottle_noise: N,
    /// Distância, em metros, entre as amostras da diferença central da inclinação.
    pub sample_step_m: f32,
    /// Inclinação (subida/avanço) abaixo da qual o peso de inclinação é zero.
    pub slope_start: f32,
    /// Inclinação (subida/avanço) a partir da qual o peso de inclinação é um.
    pub slope_end: f32,
    /// Quantos metros abaixo do entorno o peso de altura começa a subir.
    pub depth_start_m: f32,
    /// Quantos metros abaixo do entorno o peso de altura chega a um.
    pub depth_full_m: f32,
    /// Peso máximo que a altura relativa sozinha contribui (o `k_altura`).
    pub height_weight_scale: f32,
    /// Fator de mancha com o ruído em -1.
    pub mottle_min: f32,
    /// Fator de mancha com o ruído em +1.
    pub mottle_max: f32,
    /// Cor (RGBA linear) sem tingimento.
    pub flat_color: [f32; 4],
    /// Cor (RGBA linear) com tingimento total.
    pub tinted_color: [f32; 4],
}

/// Peso de 0 a 1 pela altura relativa ao entorno: 0 no nível do entorno ou acima dele, subindo
/// suavemente até 1 quando o ponto fica `depth_full_m` abaixo.
fn relative_height_weight(relative_height_m: f32, depth_start_m: f32, depth_full_m: f32) -> f32 {
    smoothstep(depth_start_m, depth_full_m, -relative_height_m)
}

/// Fator de mancha em `[min, max]` a partir de um ruído em `[-1, 1]` (valores fora são cortados).
fn mottle_factor(noise: f32, min: f32, max: f32) -> f32 {
    let t = (noise.clamp(-1.0, 1.0) + 1.0) * 0.5;
    min + (max - min) * t
}

/// Peso final de tingimento, em `[0, 1]`.
fn combined_tint_weight(
    slope_weight: f32,
    height_weight: f32,
    height_weight_scale: f32,
    mottle: f32,
) -> f32 {
    ((slope_weight + height_weight_scale * height_weight).min(1.0) * mottle).clamp(0.0, 1.0)
}

impl<S: HeightSource, B: HeightSource, N: HeightSource> TerrainTintField<S, B, N> {
    /// Inclinação (módulo do gradiente, subida/avanço) da fonte no ponto (x, z).
    fn slope_at(&self, x: f32, z: f32) -> f32 {
        let step = self.sample_step_m;
        let dx = (self.source.height_at(x + step, z) - self.source.height_at(x - step, z))
            / (2.0 * step);
        let dz = (self.source.height_at(x, z + step) - self.source.height_at(x, z - step))
            / (2.0 * step);
        dx.hypot(dz)
    }

    /// Peso de tingimento no ponto (x, z), em `[0, 1]`.
    fn tint_weight(&self, x: f32, z: f32) -> f32 {
        let slope_weight = smoothstep(self.slope_start, self.slope_end, self.slope_at(x, z));
        let relative_height = self.source.height_at(x, z) - self.smoothed_source.height_at(x, z);
        let height_weight =
            relative_height_weight(relative_height, self.depth_start_m, self.depth_full_m);
        let mottle = mottle_factor(
            self.mottle_noise.height_at(x, z),
            self.mottle_min,
            self.mottle_max,
        );
        combined_tint_weight(
            slope_weight,
            height_weight,
            self.height_weight_scale,
            mottle,
        )
    }
}

impl<S: HeightSource, B: HeightSource, N: HeightSource> ColorSource for TerrainTintField<S, B, N> {
    fn color_at(&self, x: f32, z: f32) -> [f32; 4] {
        lerp4(self.flat_color, self.tinted_color, self.tint_weight(x, z))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Constant(f32);
    impl HeightSource for Constant {
        fn height_at(&self, _x: f32, _z: f32) -> f32 {
            self.0
        }
    }

    const FLAT: [f32; 4] = [1.0, 1.0, 1.0, 1.0];
    const TINTED: [f32; 4] = [0.5, 0.4, 0.3, 1.0];

    /// Campo sobre terreno plano (sem inclinação), com altura relativa `relative_height_m` e
    /// ruído de mancha constante `noise`.
    fn flat_field(
        relative_height_m: f32,
        noise: f32,
    ) -> TerrainTintField<Constant, Constant, Constant> {
        TerrainTintField {
            source: Constant(relative_height_m),
            smoothed_source: Constant(0.0),
            mottle_noise: Constant(noise),
            sample_step_m: 10.0,
            slope_start: 0.06,
            slope_end: 0.2,
            depth_start_m: 10.0,
            depth_full_m: 45.0,
            height_weight_scale: 0.5,
            mottle_min: 0.6,
            mottle_max: 1.2,
            flat_color: FLAT,
            tinted_color: TINTED,
        }
    }

    /// Testa que o peso por altura relativa é zero no nível do entorno e acima, um a partir de
    /// `depth_full_m` abaixo, e cresce sem voltar no meio.
    #[test]
    fn relative_height_weight_grows_from_zero_at_the_surroundings_to_one_deep_below() {
        assert_eq!(relative_height_weight(30.0, 10.0, 45.0), 0.0);
        assert_eq!(relative_height_weight(0.0, 10.0, 45.0), 0.0);
        assert_eq!(relative_height_weight(-10.0, 10.0, 45.0), 0.0);
        assert_eq!(relative_height_weight(-45.0, 10.0, 45.0), 1.0);
        assert_eq!(relative_height_weight(-80.0, 10.0, 45.0), 1.0);

        let mut previous = 0.0;
        for depth in 10..=45 {
            let weight = relative_height_weight(-(depth as f32), 10.0, 45.0);
            assert!(
                weight >= previous,
                "peso caiu em {depth} m: {weight} < {previous}"
            );
            previous = weight;
        }
    }

    /// Testa que um ponto de vale (abaixo do entorno) fica mais tingido que um de crista, em
    /// terreno sem inclinação.
    #[test]
    fn a_point_below_its_surroundings_is_tinted_more_than_a_crest() {
        let valley = flat_field(-40.0, 0.0).color_at(0.0, 0.0);
        let crest = flat_field(40.0, 0.0).color_at(0.0, 0.0);

        assert_eq!(crest, FLAT);
        assert!(
            valley[0] < crest[0],
            "vale {valley:?} deveria ser mais escuro que crista"
        );
    }

    /// Testa que o fator de mancha cobre exatamente a faixa configurada e corta ruído fora de
    /// `[-1, 1]`.
    #[test]
    fn mottle_factor_maps_the_noise_range_onto_the_configured_range() {
        assert_eq!(mottle_factor(-1.0, 0.6, 1.2), 0.6);
        assert_eq!(mottle_factor(1.0, 0.6, 1.2), 1.2);
        assert!((mottle_factor(0.0, 0.6, 1.2) - 0.9).abs() < 1e-6);
        assert_eq!(mottle_factor(-3.0, 0.6, 1.2), 0.6);
        assert_eq!(mottle_factor(3.0, 0.6, 1.2), 1.2);
    }

    /// Testa que o peso final nunca passa de 1, mesmo com inclinação e altura no máximo e mancha
    /// acima de 1, então a cor nunca fica mais escura que `tinted_color`.
    #[test]
    fn combined_weight_never_exceeds_one() {
        assert_eq!(combined_tint_weight(1.0, 1.0, 0.5, 1.2), 1.0);
        assert!((combined_tint_weight(0.2, 1.0, 0.5, 1.0) - 0.7).abs() < 1e-6);
        assert_eq!(combined_tint_weight(0.0, 0.0, 0.5, 1.2), 0.0);

        let deepest = flat_field(-200.0, 1.0).color_at(0.0, 0.0);
        for (channel, tinted) in deepest.iter().zip(TINTED) {
            assert!(*channel >= tinted - 1e-6);
        }
    }
}
