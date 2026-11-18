//! Receita de terreno de Ganimedes: relevo todo procedural, sem DTM.
//!
//! O recorte representa o contato entre terreno escuro antigo, com crateras, e uma faixa de sulcos
//! claros, em latitude média do hemisfério sub-joviano (ver o comentário no topo do
//! `ganymede.ron`). Por enquanto é só a planície com a clareira de spawn; crateras, sulcos e cor
//! ainda não existem.

use super::{
    SPAWN_CLEARING_BLEND_M, SPAWN_CLEARING_RADIUS_M, TERRAIN_RES, TERRAIN_SIZE_M, TerrainRecipe,
    VERTICAL_EXAGGERATION, appearance_and_material, chunked_lod_config,
};
use crate::height::arc;
use crate::height::comb::{Add2, FlattenNearOrigin};
use crate::height::noise::PerlinFbm;
use crate::params::{TerrainParams, TerrainShape};
use bevy::prelude::Vec2;
use eigc_moons::profile::MoonProfile;
use noise::Perlin;

/// Amplitude da ondulação de grande escala do terreno escuro de Ganimedes, em metros. Valor de
/// partida, não medido, a calibrar junto com as crateras e os sulcos.
const GANYMEDE_PLAINS_AMPLITUDE_M: f32 = 40.0;

/// Oitavas da ondulação de grande escala.
const GANYMEDE_PLAINS_OCTAVES: u32 = 5;

/// Comprimento de onda da primeira oitava da rugosidade fina, em metros.
const GANYMEDE_DETAIL_BASE_WAVELENGTH_M: f32 = 230.0;

/// Oitavas da rugosidade fina: de ~230 m a ~14 m, o limite que o nível 0 da malha mostra.
const GANYMEDE_DETAIL_OCTAVES: u32 = 5;

/// Expoente de Hurst da rugosidade fina. Valor de partida: não há DTM de Ganimedes nessa escala
/// para medir. 0,5 é um perfil áspero, que deixa energia nas oitavas pequenas.
const GANYMEDE_DETAIL_HURST_EXPONENT: f32 = 0.5;

/// Amplitude da rugosidade fina (soma normalizada das oitavas), em metros. Valor de partida, a
/// calibrar com o app rodando.
const GANYMEDE_DETAIL_AMPLITUDE_M: f32 = 12.0;

/// Monta a receita de geração de terreno para Ganimedes.
///
/// Recorte quadrado, para o anel de extensão poder começar na borda quando entrar.
/// Sem cor por vértice: o terreno usa a cor lisa do perfil.
pub(super) fn ganymede_recipe(profile: &MoonProfile) -> TerrainRecipe {
    let calibration = &profile.terrain;
    let seed = calibration.seed;

    let terrain_params = TerrainParams {
        size: TERRAIN_SIZE_M,
        res: TERRAIN_RES,
        freq: calibration.base_frequency,
        line_dir: Vec2::from(calibration.feature_direction).normalize(),
        seed,
        vertical_exaggeration: VERTICAL_EXAGGERATION,
        shape: TerrainShape::Square,
    };

    let (appearance, material_properties) = appearance_and_material(profile);

    TerrainRecipe {
        height: arc(ganymede_relief(seed, calibration.base_frequency)),
        params: terrain_params,
        appearance,
        material_properties,
        color: None,
        lod: chunked_lod_config(),
        far_field: None,
        color_map: None,
    }
}

/// Relevo de Ganimedes: planície achatada na clareira de spawn, com a rugosidade fina por cima.
///
/// A rugosidade entra depois do achatamento, para a clareira ser um platô no nível
/// do terreno ao redor e não um plano perfeito.
fn ganymede_relief(
    seed: u32,
    base_frequency: f32,
) -> Add2<FlattenNearOrigin<PerlinFbm>, PerlinFbm> {
    Add2 {
        a: FlattenNearOrigin::new(
            ganymede_plains(seed, base_frequency),
            SPAWN_CLEARING_RADIUS_M,
            SPAWN_CLEARING_BLEND_M,
        ),
        b: ganymede_fine_detail(seed),
    }
}

/// Ondulação de grande escala do terreno, centrada em zero.
fn ganymede_plains(seed: u32, base_frequency: f32) -> PerlinFbm {
    PerlinFbm {
        perlin: Perlin::new(seed),
        freq: base_frequency,
        octaves: GANYMEDE_PLAINS_OCTAVES,
        lacunarity: 2.0,
        gain: 0.5,
        amplitude: GANYMEDE_PLAINS_AMPLITUDE_M,
    }
}

/// Rugosidade fina, com semente própria para não repetir o padrão da ondulação de grande escala.
fn ganymede_fine_detail(seed: u32) -> PerlinFbm {
    PerlinFbm {
        perlin: Perlin::new(seed.wrapping_add(1)),
        freq: 1.0 / GANYMEDE_DETAIL_BASE_WAVELENGTH_M,
        octaves: GANYMEDE_DETAIL_OCTAVES,
        lacunarity: 2.0,
        gain: 2.0_f32.powf(-GANYMEDE_DETAIL_HURST_EXPONENT),
        amplitude: GANYMEDE_DETAIL_AMPLITUDE_M,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::height::HeightSource;

    /// Perfil real de Ganimedes, para medir a receita com a frequência que o jogo usa.
    fn ganymede_profile() -> MoonProfile {
        ron::de::from_str(include_str!("../../../../assets/moons/ganymede.ron"))
            .expect("o ganymede.ron deveria ser um MoonProfile válido")
    }

    /// Testa que a clareira de spawn de Ganimedes é um platô com rugosidade fina: dentro do raio
    /// plano, a altura é exatamente o platô mais a rugosidade, e varia de um ponto a outro.
    #[test]
    fn ganymede_spawn_clearing_keeps_the_fine_roughness_over_the_plateau() {
        let profile = ganymede_profile();
        let seed = profile.terrain.seed;
        let recipe = ganymede_recipe(&profile);
        let plateau = FlattenNearOrigin::new(
            ganymede_plains(seed, profile.terrain.base_frequency),
            SPAWN_CLEARING_RADIUS_M,
            SPAWN_CLEARING_BLEND_M,
        )
        .flat_height;
        let detail = ganymede_fine_detail(seed);

        let mut lowest = f32::INFINITY;
        let mut highest = f32::NEG_INFINITY;
        for ring in 0..=10 {
            let radius = SPAWN_CLEARING_RADIUS_M * ring as f32 / 10.0;
            for step in 0..32 {
                let angle = step as f32 / 32.0 * std::f32::consts::TAU;
                let (x, z) = (angle.cos() * radius, angle.sin() * radius);
                let height = recipe.height.height_at(x, z);
                assert_eq!(height, plateau + detail.height_at(x, z), "({x}, {z})");
                lowest = lowest.min(height);
                highest = highest.max(height);
            }
        }
        assert!(highest - lowest > 0.1, "clareira lisa demais: {lowest} a {highest}");
    }
}
