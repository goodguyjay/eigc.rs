//! Monta a recipe de geração de terreno para cada lua suportada.
//! Esse módulo é o único lugar onde MoonId decide qual composição de HeightSource e quais
//! parâmetros visuais correspondem a uma lua específica.

use crate::height::comb::{Add2, Bias, Scale};
use crate::height::noise::{PerlinFbm, PerlinRidged};
use crate::height::warp::{Oriented, Warp2D};
use crate::height::{HeightFn, arc};
use crate::params::TerrainParams;
use crate::pipeline::TerrainAppearance;
use crate::systems::TerrainMaterialProperties;
use bevy::prelude::{Color, Vec2};
use eigc_moons::profile::{MoonId, MoonProfile};
use noise::Perlin;

/// Agrupa os três produtos de uma receita de terreno: a função de altura composta, os parâmetros
/// de malha/mundo, e a aparência visual.
pub struct TerrainRecipe {
    /// Altura do terreno
    pub height: HeightFn,
    /// Parâmetros de malha/mundo do terreno
    pub params: TerrainParams,
    /// Aparência visual do terreno
    pub appearance: TerrainAppearance,
    /// Propriedades do material do terreno
    pub material_properties: TerrainMaterialProperties,
}

/// Ponto de entrada para montar a receita de geração de terreno de uma lua específica.
pub fn build_recipe(profile: &MoonProfile) -> TerrainRecipe {
    match profile.moon_id {
        MoonId::Europa => europa_recipe(profile),
        MoonId::Io => unimplemented!("receita de IO ainda não calibrada"),
        MoonId::Ganymede => unimplemented!("receita de Ganymede ainda não calibrada"),
        MoonId::Callisto => unimplemented!("receita de Callisto ainda não calibrada"),
    }
}

/// Monta a receita de geração de terreno para Europa.
/// Combina ruído base suave com crista anisotrópica orientada ao longo da diração de lineae
fn europa_recipe(profile: &MoonProfile) -> TerrainRecipe {
    let calibration = &profile.terrain;

    let seed = calibration.seed;
    let base_frequency = calibration.base_frequency;

    let feature_direction = Vec2::from(calibration.feature_direction).normalize();

    let base_noise = PerlinFbm {
        perlin: Perlin::new(seed),
        freq: base_frequency,
        octaves: 5,
        lacunarity: 2.0,
        gain: 0.5,
        amplitude: 1.0,
    };

    let ridged_noise = PerlinRidged {
        perlin: Perlin::new(seed ^ 0xB529_7A4D),
        freq: base_frequency * 2.5,
        octaves: 4,
        lacunarity: 2.2,
        gain: 0.75,
        amplitude: 1.0,
        z_anisotropy: 2.0,
    };

    let oriented_ridges = Oriented {
        source: ridged_noise,
        dir: feature_direction,
        main_scale: 1.0,
        ortho_scale: 0.35,
    };

    let combined_features = Add2 {
        a: base_noise,
        b: oriented_ridges,
    };

    let warped_terrain = Warp2D {
        source: combined_features,
        perlin: Perlin::new(seed ^ 0x9E37_79B9),
        warp_amp: calibration.warp_amplitude_meters,
        warp_freq: base_frequency * 0.6,
        octaves: 3,
        lacunarity: 2.1,
        gain: 0.55,
    };

    let scaled_terrain = Scale {
        s: warped_terrain,
        scale: 1.0,
    };

    // bias: -0.1 é um ajuste estético. Não corresponde à precisão dos dados reais de Europa.
    // TODO (leticia.rodrigues): testar visualmente quando a cena tiver câmera/luz.
    let biased_terrain = Bias {
        s: scaled_terrain,
        bias: -0.1,
    };

    let terrain_params = TerrainParams {
        size: 3000.0,
        res: 512,
        amp: calibration.vertical_amplitude_meters,
        freq: base_frequency,
        line_dir: feature_direction,
        seed,
    };

    let terrain_appearance = TerrainAppearance {
        base_color: color_from_linear_rgba(profile.terrain_base_color),
        display_name: profile.display_name.clone(),
    };

    let material_properties = TerrainMaterialProperties {
        perceptual_roughness: calibration.perceptual_roughness,
        reflectance: calibration.reflectance,
    };

    TerrainRecipe {
        height: arc(biased_terrain),
        params: terrain_params,
        appearance: terrain_appearance,
        material_properties,
    }
}

/// Converte um array [r, g, b, a] em componentes lineares (formato salvo no .ron) para o tipo Color do bevy
fn color_from_linear_rgba(components: [f32; 4]) -> Color {
    Color::srgba(components[0], components[1], components[2], components[3])
}
