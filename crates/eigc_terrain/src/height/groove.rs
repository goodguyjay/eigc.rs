//! Faixas de sulcos paralelos, como o terreno claro sulcado (sulci) de Ganimedes: uma faixa
//! alongada, de borda irregular, cortada por sulcos e cristas paralelos à direção da faixa, de
//! espaçamento quase regular.
//!
//! A faixa soma um desvio de altura à fonte: o nível próprio da faixa (`floor_offset_m`) mais o
//! perfil dos sulcos, os dois pesados pela posição na borda. Fora da faixa, o desvio é zero.

use super::{HeightFn, HeightSource};
use bevy::prelude::Vec2;
use eigc_common::math::smoothstep;

/// Perfil de um sulco ao longo de um comprimento de onda: `phase` em comprimentos de onda, 0 no
/// topo de uma crista e 0,5 no fundo de um sulco. Varia de -1 a 1.
pub fn groove_profile(phase: f32) -> f32 {
    (phase * std::f32::consts::TAU).cos()
}

/// Faixa de sulcos paralelos.
pub struct GrooveBand {
    /// Um ponto da linha central da faixa, em metros.
    pub center_point: Vec2,
    /// Direção dos sulcos e da faixa (unitária).
    pub strike: Vec2,
    /// Meia largura da faixa, da linha central até o meio da transição da borda, em metros.
    pub half_width_m: f32,
    /// Largura da transição da borda, em metros.
    pub edge_width_m: f32,
    /// Distância entre cristas vizinhas, em metros.
    pub wavelength_m: f32,
    /// Meia amplitude dos sulcos (crista até o nível médio), em metros.
    pub amplitude_m: f32,
    /// Nível da faixa em relação ao terreno ao redor, em metros (negativo deixa a faixa mais
    /// baixa).
    pub floor_offset_m: f32,
    /// Ruído em [-1, 1] que desloca a borda da faixa.
    pub edge_noise: HeightFn,
    /// Deslocamento máximo da borda, em metros.
    pub edge_noise_amplitude_m: f32,
    /// Ruído em [-1, 1] que entorta os sulcos (desloca a fase).
    pub bend_noise: HeightFn,
    /// Deslocamento máximo dos sulcos pela torção, em metros.
    pub bend_noise_amplitude_m: f32,
    /// Ruído em [-1, 1] que varia a amplitude dos sulcos ao longo deles.
    pub amplitude_noise: HeightFn,
    /// Variação máxima da amplitude, em fração dela.
    pub amplitude_variation: f32,
}

impl GrooveBand {
    /// Normal da faixa no plano (perpendicular aos sulcos).
    fn normal(&self) -> Vec2 {
        self.strike.normalize().perp()
    }

    /// Distância do ponto à linha central, com sinal, em metros.
    pub fn across_m(&self, x: f32, z: f32) -> f32 {
        (Vec2::new(x, z) - self.center_point).dot(self.normal())
    }

    /// Peso da faixa no ponto: 1 dentro, 0 fora, com a transição na borda irregular.
    pub fn band_weight_at(&self, x: f32, z: f32) -> f32 {
        let edge = self.half_width_m
            + self.edge_noise.height_at(x, z).clamp(-1.0, 1.0) * self.edge_noise_amplitude_m;
        let distance = self.across_m(x, z).abs();
        1.0 - smoothstep(
            edge - 0.5 * self.edge_width_m,
            edge + 0.5 * self.edge_width_m,
            distance,
        )
    }
}

impl HeightSource for GrooveBand {
    fn height_at(&self, x: f32, z: f32) -> f32 {
        let weight = self.band_weight_at(x, z);
        if weight <= 0.0 {
            return 0.0;
        }
        let bend = self.bend_noise.height_at(x, z).clamp(-1.0, 1.0) * self.bend_noise_amplitude_m;
        let phase = (self.across_m(x, z) + bend) / self.wavelength_m;
        let variation = self.amplitude_noise.height_at(x, z).clamp(-1.0, 1.0);
        let amplitude = self.amplitude_m * (1.0 + self.amplitude_variation * variation);
        weight * (self.floor_offset_m + amplitude * groove_profile(phase))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::height::arc;

    struct Zero;
    impl HeightSource for Zero {
        fn height_at(&self, _x: f32, _z: f32) -> f32 {
            0.0
        }
    }

    /// Faixa ao longo de Z, centrada em x = 0, sem ruído.
    fn straight_band() -> GrooveBand {
        GrooveBand {
            center_point: Vec2::ZERO,
            strike: Vec2::Y,
            half_width_m: 3000.0,
            edge_width_m: 600.0,
            wavelength_m: 2000.0,
            amplitude_m: 100.0,
            floor_offset_m: -50.0,
            edge_noise: arc(Zero),
            edge_noise_amplitude_m: 500.0,
            bend_noise: arc(Zero),
            bend_noise_amplitude_m: 300.0,
            amplitude_noise: arc(Zero),
            amplitude_variation: 0.3,
        }
    }

    /// Testa que o perfil do sulco tem crista em 0, fundo em 0,5 e repete a cada comprimento de
    /// onda.
    #[test]
    fn groove_profile_has_a_crest_at_zero_and_a_trough_at_half() {
        assert!((groove_profile(0.0) - 1.0).abs() < 1e-6);
        assert!((groove_profile(0.5) + 1.0).abs() < 1e-6);
        assert!((groove_profile(0.3) - groove_profile(1.3)).abs() < 1e-5);
    }

    /// Testa que fora da faixa o desvio é zero e dentro dela os sulcos se repetem a cada
    /// comprimento de onda na direção perpendicular, sem variar ao longo dos sulcos.
    #[test]
    fn grooves_repeat_across_the_band_and_vanish_outside_it() {
        let band = straight_band();
        assert_eq!(band.height_at(5000.0, 0.0), 0.0);
        assert_eq!(band.height_at(-5000.0, 1234.0), 0.0);

        let crest = band.height_at(0.0, 0.0);
        assert!((crest - (-50.0 + 100.0)).abs() < 1e-3, "crista {crest}");
        assert!((band.height_at(2000.0, 0.0) - crest).abs() < 1e-3);
        assert!((band.height_at(0.0, 7000.0) - crest).abs() < 1e-3);
        assert!((band.height_at(1000.0, 0.0) - (-50.0 - 100.0)).abs() < 1e-3);
    }

    /// Testa que o peso da faixa cai de 1 a 0 na borda, e que o ruído de borda a desloca.
    #[test]
    fn band_weight_drops_across_the_edge_and_follows_the_edge_noise() {
        let band = straight_band();
        assert_eq!(band.band_weight_at(2600.0, 0.0), 1.0);
        assert!((band.band_weight_at(3000.0, 0.0) - 0.5).abs() < 1e-5);
        assert_eq!(band.band_weight_at(3400.0, 0.0), 0.0);

        struct One;
        impl HeightSource for One {
            fn height_at(&self, _x: f32, _z: f32) -> f32 {
                1.0
            }
        }
        let widened = GrooveBand {
            edge_noise: arc(One),
            ..straight_band()
        };
        assert!((widened.band_weight_at(3500.0, 0.0) - 0.5).abs() < 1e-5);
    }
}
