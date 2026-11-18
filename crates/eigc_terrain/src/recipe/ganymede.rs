//! Receita de terreno de Ganimedes: relevo todo procedural, sem DTM.
//!
//! O recorte representa o contato entre terreno escuro antigo e uma faixa de terreno claro
//! sulcado (sulcus), em latitude média do hemisfério sub-joviano (ver o comentário no topo do
//! `ganymede.ron`). O terreno escuro é coberto de crateras degradadas; a faixa clara é mais
//! jovem, cortada por sulcos paralelos, e tem poucas crateras, mais conservadas. A clareira de
//! spawn fica no terreno escuro, a cerca de 1 km do contato.

use super::{
    SPAWN_CLEARING_BLEND_M, SPAWN_CLEARING_RADIUS_M, TERRAIN_RES, TERRAIN_SIZE_M, TerrainRecipe,
    VERTICAL_EXAGGERATION, appearance_and_material, chunked_lod_config,
};
use crate::color_map::ColorMapConfig;
use crate::far_field::FarFieldConfig;
use crate::height::comb::{Add2, FlattenColorNearOrigin, FlattenNearOrigin};
use crate::height::crater::{
    CraterColorField, CraterField, CraterScale, CraterSpec, generate_crater_specs,
};
use crate::height::groove::{GrooveBand, GrooveBandColor};
use crate::height::noise::PerlinFbm;
use crate::height::slope::{PoleFacingFrost, SlopeColorField};
use crate::height::{ColorSource, arc, arc_color};
use crate::lod::TerrainLodConfig;
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

/// Distância da origem até a linha central da faixa de sulcos, em metros. Com a meia largura, a
/// borda e o ruído de borda abaixo, a faixa começa a ~1 km da clareira de spawn.
const GANYMEDE_BAND_OFFSET_M: f32 = 4500.0;

/// Meia largura da faixa de sulcos, em metros. As faixas reais têm dezenas a centenas de km;
/// aqui a faixa cobre ~5 km para caber o contato com o terreno escuro no recorte.
const GANYMEDE_BAND_HALF_WIDTH_M: f32 = 2500.0;

/// Largura da transição na borda da faixa, em metros.
const GANYMEDE_BAND_EDGE_WIDTH_M: f32 = 800.0;

/// Deslocamento máximo da borda da faixa pelo ruído, em metros.
const GANYMEDE_BAND_EDGE_NOISE_M: f32 = 600.0;

/// Comprimento de onda do ruído da borda da faixa, em metros.
const GANYMEDE_BAND_EDGE_NOISE_WAVELENGTH_M: f32 = 3000.0;

/// Nível da faixa clara em relação ao terreno escuro, em metros. Valor de partida: o terreno
/// sulcado se formou por extensão da crosta e costuma ficar um pouco mais baixo.
const GANYMEDE_BAND_FLOOR_OFFSET_M: f32 = -40.0;

/// Distância entre cristas vizinhas dos sulcos, em metros. Os sulcos reais mais marcados ficam
/// entre 3 e 10 km, com sulcos menores, de ~1 km, por cima; aqui 1,2 km, para caberem vários na
/// faixa. Valor de partida.
const GANYMEDE_GROOVE_WAVELENGTH_M: f32 = 1200.0;

/// Meia amplitude dos sulcos, em metros (~120 m de crista a fundo). Dá inclinação máxima de
/// ~0,31 (~17 graus) nas encostas dos sulcos. Valor de partida.
const GANYMEDE_GROOVE_AMPLITUDE_M: f32 = 60.0;

/// Deslocamento máximo dos sulcos pela torção, em metros.
const GANYMEDE_GROOVE_BEND_M: f32 = 250.0;

/// Comprimento de onda do ruído de torção dos sulcos, em metros.
const GANYMEDE_GROOVE_BEND_WAVELENGTH_M: f32 = 4000.0;

/// Variação da amplitude dos sulcos ao longo deles, em fração da amplitude.
const GANYMEDE_GROOVE_AMPLITUDE_VARIATION: f32 = 0.4;

/// Comprimento de onda do ruído que varia a amplitude dos sulcos, em metros.
const GANYMEDE_GROOVE_AMPLITUDE_NOISE_WAVELENGTH_M: f32 = 3000.0;

/// Tentativas de sorteio de crateras. Com a densidade abaixo, sobram ~350 no terreno escuro.
/// Pela contagem de crateras do terreno escuro (ordem de 100 a 200 crateras acima de 10 km por
/// milhão de km²) estendida pela lei de potência, o recorte teria algumas centenas acima de
/// ~160 m de diâmetro. Valor de partida.
const GANYMEDE_CRATER_ATTEMPTS: usize = 500;

/// Sal somado à semente do perfil para sortear as crateras. Escolhido entre os sais de 5 a 24
/// porque, com a semente do `ganymede.ron`, deixa uma cratera complexa de ~3,3 km de diâmetro a
/// ~5,5 km a leste da clareira (do lado do nascer do sol, no terreno escuro). Com a lei de
/// potência, 500 tentativas dão em média menos de três crateras acima de 1 km de raio, e alguns
/// sais não dão nenhuma.
const GANYMEDE_CRATER_SEED_SALT: u32 = 11;

/// Densidade relativa de crateras na faixa clara: a faixa é mais jovem e tem várias vezes menos
/// crateras que o terreno escuro. Valor de partida.
const GANYMEDE_BAND_CRATER_DENSITY: f32 = 0.12;

/// Medidas das crateras de Ganimedes. Crateras recentes pequenas são tigelas com profundidade
/// de ~0,2 do diâmetro; as maiores do recorte são mais rasas, com piso plano e pico central (a
/// transição simples/complexa em luas de gelo fica perto de 2 a 3 km de diâmetro). As crateras
/// de fosso central de Ganimedes passam de ~35 km e não cabem no recorte. Valores de partida.
const GANYMEDE_CRATER_SCALE: CraterScale = CraterScale {
    radius_min_m: 80.0,
    radius_max_m: 2800.0,
    size_exponent: 2.0,
    depth_ratio_small: 0.2,
    depth_ratio_large: 0.12,
    rim_to_depth: 0.2,
    complex_radius_m: 1500.0,
    complex_floor_fraction: 0.4,
    central_peak_to_depth: 0.3,
    central_peak_radius_fraction: 0.15,
    degraded_depth_fraction: 0.3,
    degraded_rim_fraction: 0.15,
};

/// Distância entre as amostras usadas para estimar a inclinação na cor, em metros. Menor que a
/// cratera mínima (~80 m de raio), para as paredes das crateras pequenas aparecerem.
const GANYMEDE_SLOPE_SAMPLE_STEP_M: f32 = 20.0;

/// Inclinação abaixo da qual o terreno fica só com a cor de fundo. Medida na superfície com o
/// `ganymede.ron` e passo de 20 m (ver `slope_color_thresholds_follow_the_measured_surface_slope`):
/// mediana ~0,03 (planície), percentil 80 ~0,20 (sulcos e crateras), percentil 95 ~0,35 e máximo
/// ~1,1 (paredes de crateras recentes). Começa acima do percentil 80, para o gelo claro ficar nas
/// encostas mais íngremes sem listrar a faixa inteira.
const GANYMEDE_SLOPE_COLOR_START: f32 = 0.22;

/// Inclinação a partir da qual o terreno fica com a cor de encosta (gelo exposto) plena.
const GANYMEDE_SLOPE_COLOR_END: f32 = 0.5;

/// Terreno claro sulcado: gelo mais limpo, mais claro e menos avermelhado que o terreno escuro
/// (RGBA linear). Valor de partida.
const GANYMEDE_BAND_COLOR: [f32; 4] = [0.72, 0.72, 0.70, 1.0];

/// Gelo exposto no interior e no ejecta das crateras recentes (RGBA linear).
const GANYMEDE_FRESH_ICE_COLOR: [f32; 4] = [0.86, 0.86, 0.86, 1.0];

/// Conservação a partir da qual a cratera clareia o entorno. Com a distribuição da receita, só
/// uma minoria das crateras do terreno escuro passa daqui; na faixa, a maioria.
const GANYMEDE_FRESH_CRATER_MIN_FRESHNESS: f32 = 0.75;

/// Geada nas encostas voltadas para o polo (RGBA linear), levemente azulada.
const GANYMEDE_FROST_COLOR: [f32; 4] = [0.90, 0.91, 0.94, 1.0];

/// Inclinação voltada para o polo em que a geada começa. O terreno é quase todo suave
/// (inclinação mediana ~0,03), então a geada já começa em encostas suaves voltadas para o norte;
/// calibrado pela cobertura (ver `frost_covers_a_minority_of_the_cutout`).
const GANYMEDE_FROST_FACING_START: f32 = 0.02;

/// Inclinação voltada para o polo a partir da qual a geada é plena (antes das manchas).
const GANYMEDE_FROST_FACING_END: f32 = 0.08;

/// Comprimento de onda da máscara que quebra a geada em manchas, em metros.
const GANYMEDE_FROST_MASK_WAVELENGTH_M: f32 = 1500.0;

/// Valor da máscara da geada (ruído em [-1, 1]) abaixo do qual não há geada.
const GANYMEDE_FROST_MASK_START: f32 = -0.5;

/// Valor da máscara da geada a partir do qual a geada não é atenuada.
const GANYMEDE_FROST_MASK_END: f32 = 0.0;

/// Raio médio de Ganimedes, em metros, usado na queda da curvatura do anel de extensão.
const GANYMEDE_RADIUS_M: f32 = 2_634_100.0;

/// Anel de extensão: um bloco de 22 km em cada direção (até 33 km do centro, dentro do `far` de
/// 50 km da câmera). 128 quads por bloco dão ~172 m entre vértices: a faixa de sulcos (a cada
/// 1,2 km) continua visível no anel; as crateras só existem dentro do recorte.
const GANYMEDE_FAR_FIELD: FarFieldConfig = FarFieldConfig {
    rings: 1,
    quads_per_tile: 128,
    moon_radius_m: GANYMEDE_RADIUS_M,
    skirt_depth_factor: 0.35,
};

/// Mapa de cor: 4096 texels nos 22 km do recorte dão ~5,4 m por texel, o espaçamento do nível
/// mais fino do LOD. Sem ele, o nível mais grosso (~43 m) perderia os raios das crateras e as
/// manchas de geada.
const GANYMEDE_COLOR_MAP: ColorMapConfig = ColorMapConfig {
    texels_per_side: 4096,
};

/// Piso do erro do LOD como fração do espaçamento do nível (o padrão é 0,2). Com o mapa de cor, a
/// cor não perde resolução quando o chunk engrossa, e o piso só protege a geometria.
const GANYMEDE_MIN_ERROR_FRACTION: f32 = 0.1;

/// Direção horizontal para o polo norte no plano XZ (o norte é -Z no mundo).
const NORTH: Vec2 = Vec2::new(0.0, -1.0);

/// Monta a receita de geração de terreno para Ganimedes.
///
/// Recorte quadrado, com anel de extensão a partir da borda. A cor parte do terreno escuro, com
/// gelo claro nas encostas íngremes; a faixa sulcada pinta o terreno claro, as crateras recentes
/// clareiam o interior, o ejecta e os raios, e as encostas voltadas para o polo guardam geada.
pub(super) fn ganymede_recipe(profile: &MoonProfile) -> TerrainRecipe {
    let calibration = &profile.terrain;
    let seed = calibration.seed;
    let strike = Vec2::from(calibration.feature_direction).normalize();

    let terrain_params = TerrainParams {
        size: TERRAIN_SIZE_M,
        res: TERRAIN_RES,
        freq: calibration.base_frequency,
        line_dir: strike,
        seed,
        vertical_exaggeration: VERTICAL_EXAGGERATION,
        shape: TerrainShape::Square,
    };

    let (appearance, material_properties) = appearance_and_material(profile);

    TerrainRecipe {
        height: arc(ganymede_relief(seed, calibration.base_frequency, strike)),
        params: terrain_params,
        appearance,
        material_properties,
        color: Some(arc_color(ganymede_color(profile, strike))),
        lod: TerrainLodConfig {
            min_error_fraction: GANYMEDE_MIN_ERROR_FRACTION,
            ..chunked_lod_config()
        },
        far_field: Some(GANYMEDE_FAR_FIELD),
        color_map: Some(GANYMEDE_COLOR_MAP),
    }
}

/// Cor de Ganimedes, na ordem em que as camadas se sobrepõem: inclinação (terreno escuro e gelo
/// exposto), faixa clara, crateras recentes, geada; e a clareira de spawn com a cor de fundo.
fn ganymede_color(profile: &MoonProfile, strike: Vec2) -> impl ColorSource + use<> {
    let seed = profile.terrain.seed;
    let base_frequency = profile.terrain.base_frequency;

    let slope = SlopeColorField {
        source: ganymede_cratered_surface(seed, base_frequency, strike),
        sample_step_m: GANYMEDE_SLOPE_SAMPLE_STEP_M,
        flat_color: profile.terrain_base_color,
        steep_color: profile.terrain_steep_color,
        slope_start: GANYMEDE_SLOPE_COLOR_START,
        slope_end: GANYMEDE_SLOPE_COLOR_END,
    };
    let band = GrooveBandColor {
        base: slope,
        band: ganymede_groove_band(seed, strike),
        band_color: GANYMEDE_BAND_COLOR,
    };
    let craters = CraterColorField::new(
        band,
        ganymede_craters(seed, strike),
        GANYMEDE_FRESH_ICE_COLOR,
        GANYMEDE_FRESH_CRATER_MIN_FRESHNESS,
        seed.wrapping_add(6),
    );
    FlattenColorNearOrigin {
        source: ganymede_frost(craters, seed, base_frequency, strike),
        flat_color: profile.terrain_base_color,
        flat_radius_m: SPAWN_CLEARING_RADIUS_M,
        blend_radius_m: SPAWN_CLEARING_BLEND_M,
    }
}

/// Superfície antes da clareira: terreno escuro ondulado, a faixa de sulcos e as crateras por
/// cima das duas.
type CrateredSurface = CraterField<Add2<PerlinFbm, GrooveBand>>;

/// Relevo de Ganimedes: a superfície com crateras, achatada na clareira de spawn, com a
/// rugosidade fina por cima.
///
/// As crateras entram antes do achatamento, mas nenhuma alcança a clareira. A rugosidade entra
/// depois, para a clareira ser um platô no nível do terreno ao redor e não um plano perfeito.
fn ganymede_relief(
    seed: u32,
    base_frequency: f32,
    strike: Vec2,
) -> Add2<FlattenNearOrigin<CrateredSurface>, PerlinFbm> {
    Add2 {
        a: FlattenNearOrigin::new(
            ganymede_cratered_surface(seed, base_frequency, strike),
            SPAWN_CLEARING_RADIUS_M,
            SPAWN_CLEARING_BLEND_M,
        ),
        b: ganymede_fine_detail(seed),
    }
}

/// Terreno escuro ondulado mais a faixa de sulcos, com as crateras somadas.
fn ganymede_cratered_surface(seed: u32, base_frequency: f32, strike: Vec2) -> CrateredSurface {
    CraterField::new(
        Add2 {
            a: ganymede_plains(seed, base_frequency),
            b: ganymede_groove_band(seed, strike),
        },
        ganymede_craters(seed, strike),
    )
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

/// Ruído suave em [-1, 1] com semente própria (`salt`) e comprimento de onda dado.
fn unit_noise(seed: u32, salt: u32, wavelength_m: f32) -> PerlinFbm {
    PerlinFbm {
        perlin: Perlin::new(seed.wrapping_add(salt)),
        freq: 1.0 / wavelength_m,
        octaves: 3,
        lacunarity: 2.0,
        gain: 0.5,
        amplitude: 1.0,
    }
}

/// Faixa de sulcos paralela a `strike`, com a linha central a `GANYMEDE_BAND_OFFSET_M` da origem.
fn ganymede_groove_band(seed: u32, strike: Vec2) -> GrooveBand {
    GrooveBand {
        center_point: strike.perp() * GANYMEDE_BAND_OFFSET_M,
        strike,
        half_width_m: GANYMEDE_BAND_HALF_WIDTH_M,
        edge_width_m: GANYMEDE_BAND_EDGE_WIDTH_M,
        wavelength_m: GANYMEDE_GROOVE_WAVELENGTH_M,
        amplitude_m: GANYMEDE_GROOVE_AMPLITUDE_M,
        floor_offset_m: GANYMEDE_BAND_FLOOR_OFFSET_M,
        edge_noise: arc(unit_noise(seed, 2, GANYMEDE_BAND_EDGE_NOISE_WAVELENGTH_M)),
        edge_noise_amplitude_m: GANYMEDE_BAND_EDGE_NOISE_M,
        bend_noise: arc(unit_noise(seed, 3, GANYMEDE_GROOVE_BEND_WAVELENGTH_M)),
        bend_noise_amplitude_m: GANYMEDE_GROOVE_BEND_M,
        amplitude_noise: arc(unit_noise(seed, 4, GANYMEDE_GROOVE_AMPLITUDE_NOISE_WAVELENGTH_M)),
        amplitude_variation: GANYMEDE_GROOVE_AMPLITUDE_VARIATION,
    }
}

/// Crateras do recorte: densas e degradadas no terreno escuro, raras e mais conservadas na faixa
/// clara. Nenhuma alcança a clareira de spawn (platô mais transição).
fn ganymede_craters(seed: u32, strike: Vec2) -> Vec<CraterSpec> {
    let band = ganymede_groove_band(seed, strike);
    let band_weight = |center: Vec2| band.band_weight_at(center.x, center.y);
    generate_crater_specs(
        seed.wrapping_add(GANYMEDE_CRATER_SEED_SALT),
        TERRAIN_SIZE_M * 0.5,
        GANYMEDE_CRATER_ATTEMPTS,
        SPAWN_CLEARING_RADIUS_M + SPAWN_CLEARING_BLEND_M,
        &GANYMEDE_CRATER_SCALE,
        |center| 1.0 + (GANYMEDE_BAND_CRATER_DENSITY - 1.0) * band_weight(center),
        // no terreno escuro a maioria é antiga (u^2); na faixa, a maioria é recente (u^0,25)
        |center, u| u.powf(2.0 - 1.75 * band_weight(center)),
    )
}

/// Geada nas encostas da superfície voltadas para o norte, quebrada em manchas, por cima de
/// `base`.
fn ganymede_frost<C: ColorSource>(
    base: C,
    seed: u32,
    base_frequency: f32,
    strike: Vec2,
) -> PoleFacingFrost<C, CrateredSurface, PerlinFbm> {
    PoleFacingFrost {
        base,
        source: ganymede_cratered_surface(seed, base_frequency, strike),
        sample_step_m: GANYMEDE_SLOPE_SAMPLE_STEP_M,
        pole: NORTH,
        frost_color: GANYMEDE_FROST_COLOR,
        facing_start: GANYMEDE_FROST_FACING_START,
        facing_end: GANYMEDE_FROST_FACING_END,
        mask: unit_noise(seed, 7, GANYMEDE_FROST_MASK_WAVELENGTH_M),
        mask_start: GANYMEDE_FROST_MASK_START,
        mask_end: GANYMEDE_FROST_MASK_END,
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

    /// Semente e direção dos sulcos do perfil real.
    fn seed_and_strike() -> (u32, Vec2) {
        let profile = ganymede_profile();
        (
            profile.terrain.seed,
            Vec2::from(profile.terrain.feature_direction).normalize(),
        )
    }

    /// Testa que a clareira de spawn de Ganimedes é um platô com rugosidade fina: dentro do raio
    /// plano, a altura é exatamente o platô mais a rugosidade, e varia de um ponto a outro.
    #[test]
    fn ganymede_spawn_clearing_keeps_the_fine_roughness_over_the_plateau() {
        let profile = ganymede_profile();
        let (seed, strike) = seed_and_strike();
        let recipe = ganymede_recipe(&profile);
        let plateau = FlattenNearOrigin::new(
            ganymede_cratered_surface(seed, profile.terrain.base_frequency, strike),
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

    /// Testa que nem a faixa de sulcos nem as crateras alcançam a clareira de spawn: em todo o
    /// alcance dela (platô mais transição), a superfície é só a ondulação do terreno escuro.
    #[test]
    fn grooves_and_craters_leave_the_spawn_clearing_alone() {
        let profile = ganymede_profile();
        let base_frequency = profile.terrain.base_frequency;
        let reach = SPAWN_CLEARING_RADIUS_M + SPAWN_CLEARING_BLEND_M;
        for seed in [profile.terrain.seed, 1, 42, 999] {
            let (_, strike) = seed_and_strike();
            let plains = ganymede_plains(seed, base_frequency);
            let surface = ganymede_cratered_surface(seed, base_frequency, strike);
            for ring in 0..=14 {
                let radius = reach * ring as f32 / 14.0;
                for step in 0..64 {
                    let angle = step as f32 / 64.0 * std::f32::consts::TAU;
                    let (x, z) = (angle.cos() * radius, angle.sin() * radius);
                    assert_eq!(
                        surface.height_at(x, z),
                        plains.height_at(x, z),
                        "seed {seed}: feição dentro da clareira em ({x}, {z})"
                    );
                }
            }
        }
    }

    /// Testa que a faixa de sulcos passa pelo recorte com vários sulcos: ao longo da normal, no
    /// miolo da faixa, o desvio da faixa cruza o próprio nível médio pelo menos cinco vezes (três
    /// sulcos).
    #[test]
    fn the_groove_band_crosses_the_cutout_with_several_grooves() {
        let (seed, strike) = seed_and_strike();
        let band = ganymede_groove_band(seed, strike);
        let normal = strike.perp();
        let center = normal * GANYMEDE_BAND_OFFSET_M;
        let half = GANYMEDE_BAND_HALF_WIDTH_M - GANYMEDE_BAND_EDGE_NOISE_M;
        let samples: Vec<f32> = (0..=400)
            .map(|i| {
                let p = center + normal * (-half + 2.0 * half * i as f32 / 400.0);
                band.height_at(p.x, p.y) - GANYMEDE_BAND_FLOOR_OFFSET_M
            })
            .collect();
        let crossings = samples.windows(2).filter(|pair| pair[0].signum() != pair[1].signum());
        assert!(crossings.count() >= 5);
        assert!(center.length() + GANYMEDE_BAND_HALF_WIDTH_M < TERRAIN_SIZE_M * 0.5 + 2000.0);
    }

    /// Testa que as crateras são densas no terreno escuro e raras na faixa clara (por km²), e que
    /// o recorte tem pelo menos uma cratera grande, com mais de 1 km de raio.
    #[test]
    fn craters_are_dense_on_dark_terrain_and_sparse_in_the_band() {
        let (seed, strike) = seed_and_strike();
        let band = ganymede_groove_band(seed, strike);
        let craters = ganymede_craters(seed, strike);

        let mut band_area = 0.0;
        let steps = 110;
        let cell = TERRAIN_SIZE_M / steps as f32;
        for i in 0..steps {
            for j in 0..steps {
                let x = -TERRAIN_SIZE_M * 0.5 + (i as f32 + 0.5) * cell;
                let z = -TERRAIN_SIZE_M * 0.5 + (j as f32 + 0.5) * cell;
                if band.band_weight_at(x, z) > 0.5 {
                    band_area += cell * cell;
                }
            }
        }
        let dark_area = TERRAIN_SIZE_M * TERRAIN_SIZE_M - band_area;
        let in_band = craters
            .iter()
            .filter(|c| band.band_weight_at(c.center.x, c.center.y) > 0.5)
            .count() as f32;
        let in_dark = craters.len() as f32 - in_band;
        let band_density = in_band / band_area;
        let dark_density = in_dark / dark_area;
        println!("crateras: {in_dark} no escuro, {in_band} na faixa");

        assert!(in_dark > 200.0, "poucas crateras no terreno escuro: {in_dark}");
        assert!(band_density < 0.35 * dark_density, "{band_density} contra {dark_density}");
        assert!(craters.iter().any(|c| c.radius_m > 1000.0), "nenhuma cratera grande");
    }

    /// Pontos de uma grade de `n` por `n` que cobre o recorte (centro de cada célula).
    fn grid(n: usize) -> impl Iterator<Item = (f32, f32)> {
        let cell = TERRAIN_SIZE_M / n as f32;
        (0..n * n).map(move |k| {
            let x = -TERRAIN_SIZE_M * 0.5 + ((k % n) as f32 + 0.5) * cell;
            let z = -TERRAIN_SIZE_M * 0.5 + ((k / n) as f32 + 0.5) * cell;
            (x, z)
        })
    }

    /// Testa que os limiares da cor de encosta acompanham a inclinação medida da superfície: a
    /// mediana (planície) fica abaixo do início, o percentil 95 passa do início (há encostas
    /// claras) e o máximo passa do fim (as paredes mais íngremes saturam).
    #[test]
    fn slope_color_thresholds_follow_the_measured_surface_slope() {
        let profile = ganymede_profile();
        let (seed, strike) = seed_and_strike();
        let surface = ganymede_cratered_surface(seed, profile.terrain.base_frequency, strike);
        let step = GANYMEDE_SLOPE_SAMPLE_STEP_M;
        let mut slopes: Vec<f32> = grid(200)
            .map(|(x, z)| {
                let dx = (surface.height_at(x + step, z) - surface.height_at(x - step, z))
                    / (2.0 * step);
                let dz = (surface.height_at(x, z + step) - surface.height_at(x, z - step))
                    / (2.0 * step);
                dx.hypot(dz)
            })
            .collect();
        slopes.sort_by(f32::total_cmp);
        let at = |q: f32| slopes[((slopes.len() - 1) as f32 * q) as usize];
        let (median, p95, max) = (at(0.5), at(0.95), at(1.0));
        println!("inclinação de Ganimedes: mediana {median}, p95 {p95}, máximo {max}");

        assert!(median < GANYMEDE_SLOPE_COLOR_START, "mediana {median}");
        assert!(p95 > GANYMEDE_SLOPE_COLOR_START, "p95 {p95}");
        assert!(max > GANYMEDE_SLOPE_COLOR_END, "máximo {max}");
    }

    /// Testa que as crateras recentes (que clareiam o entorno) são minoria no terreno escuro, e
    /// que as crateras da faixa clara, mais jovem, são em média bem mais conservadas. A faixa tem
    /// poucas crateras (~13), por isso a comparação é pela média e não por uma fração.
    #[test]
    fn fresh_craters_are_rare_on_dark_terrain_and_common_in_the_band() {
        let (seed, strike) = seed_and_strike();
        let band = ganymede_groove_band(seed, strike);
        let (in_band, in_dark): (Vec<CraterSpec>, Vec<CraterSpec>) = ganymede_craters(seed, strike)
            .into_iter()
            .partition(|c| band.band_weight_at(c.center.x, c.center.y) > 0.5);
        let fresh_fraction = |craters: &[CraterSpec]| {
            let fresh = craters
                .iter()
                .filter(|c| c.freshness >= GANYMEDE_FRESH_CRATER_MIN_FRESHNESS)
                .count();
            fresh as f32 / craters.len() as f32
        };
        let mean = |craters: &[CraterSpec]| {
            craters.iter().map(|c| c.freshness).sum::<f32>() / craters.len() as f32
        };
        let dark = fresh_fraction(&in_dark);
        let (dark_mean, band_mean) = (mean(&in_dark), mean(&in_band));
        println!("recentes no escuro: {dark}; conservação média {dark_mean} contra {band_mean}");

        assert!(dark < 0.25, "recentes no escuro: {dark}");
        assert!(band_mean > dark_mean + 0.3, "{band_mean} contra {dark_mean}");
    }

    struct Black;
    impl ColorSource for Black {
        fn color_at(&self, _x: f32, _z: f32) -> [f32; 4] {
            [0.0, 0.0, 0.0, 1.0]
        }
    }

    /// Testa que a geada cobre uma minoria do recorte (entre 3% e 25% com mais da metade da cor de
    /// geada): visível nas encostas voltadas para o norte, sem tomar o terreno.
    #[test]
    fn frost_covers_a_minority_of_the_cutout() {
        let profile = ganymede_profile();
        let (seed, strike) = seed_and_strike();
        let frost = ganymede_frost(Black, seed, profile.terrain.base_frequency, strike);
        let samples = 150 * 150;
        let covered = grid(150)
            .filter(|&(x, z)| frost.color_at(x, z)[0] > 0.5 * GANYMEDE_FROST_COLOR[0])
            .count();
        let fraction = covered as f32 / samples as f32;
        println!("cobertura de geada: {fraction}");
        assert!((0.03..0.25).contains(&fraction), "cobertura {fraction}");
    }
}
