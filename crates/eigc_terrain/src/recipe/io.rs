//! Receita de terreno de Io: relevo todo procedural, sem DTM.

use super::{
    SPAWN_CLEARING_BLEND_M, SPAWN_CLEARING_RADIUS_M, TERRAIN_RES, TERRAIN_SIZE_M, TerrainRecipe,
    VERTICAL_EXAGGERATION, appearance_and_material, chunked_lod_config,
};
use crate::color_map::ColorMapConfig;
use crate::far_field::FarFieldConfig;
use crate::height::comb::{Add2, FlattenColorNearOrigin, FlattenNearOrigin};
use crate::height::noise::{PerlinFbm, PerlinRidged};
use crate::height::patches::ColorPatches;
use crate::height::patera::{
    CarvePaterae, OutlineJitter, PateraColorField, PateraScale, PateraSpec, generate_patera_specs,
};
use crate::height::slope::SlopeColorField;
use crate::height::{arc, arc_color};
use crate::params::{TerrainParams, TerrainShape};
use bevy::prelude::Vec2;
use eigc_moons::profile::MoonProfile;
use noise::Perlin;

/// Amplitude da ondulação de grande escala das planícies de Io, em metros. As planícies de Io
/// estão entre as superfícies mais lisas das luas galileanas; valor de partida, não medido.
const IO_PLAINS_AMPLITUDE_M: f32 = 40.0;

/// Oitavas da ondulação de grande escala das planícies de Io.
const IO_PLAINS_OCTAVES: u32 = 5;

/// Comprimento de onda da primeira oitava da rugosidade fina de Io, em metros.
const IO_DETAIL_BASE_WAVELENGTH_M: f32 = 230.0;

/// Oitavas da rugosidade fina de Io: de ~230 m a ~14 m. A última oitava tem ~2,7 vértices por
/// comprimento de onda no nível 0 da malha (~5,4 m entre vértices), perto do limite que a malha
/// consegue mostrar; abaixo dela a rugosidade fica para a textura de superfície.
const IO_DETAIL_OCTAVES: u32 = 5;

/// Expoente de Hurst da rugosidade fina de Io: cada oitava tem `2^-H` da amplitude da anterior.
/// Não há DTM de Io para medir; 0,5 é um perfil áspero, como o de derrames de lava, e deixa
/// energia nas oitavas pequenas (com o `gain` de 0,5, a oitava de ~14 m pesava 1/31 e não
/// aparecia). Valor de partida, a calibrar com o app rodando.
const IO_DETAIL_HURST_EXPONENT: f32 = 0.5;

/// Amplitude da rugosidade fina de Io (soma normalizada das oitavas), em metros. Calibrada para a
/// diferença rms de altura das planícies entre pontos a 29 m ficar perto de 2 m, a mesma ordem do
/// relevo de Europa nessa escala (ver `io_plains_roughness_reaches_the_target_at_small_scale`).
const IO_DETAIL_AMPLITUDE_M: f32 = 12.0;

/// Distância entre as amostras usadas para estimar a inclinação na cor de Io, em metros.
const IO_SLOPE_SAMPLE_STEP_M: f32 = 100.0;

/// Inclinação abaixo da qual o terreno de Io recebe só a cor de planície. Medida nas planícies
/// com o `io.ron` (ver `io_slope_color_thresholds_follow_the_measured_plains_slope`): mediana
/// ~0,016, percentil 95 ~0,033 e máximo ~0,06. Começa logo acima da mediana, como em Europa.
const IO_SLOPE_COLOR_START: f32 = 0.02;

/// Inclinação a partir da qual o terreno de Io recebe totalmente a cor de encosta. Fica entre o
/// percentil 95 e o máximo, para só as encostas mais íngremes saturarem.
const IO_SLOPE_COLOR_END: f32 = 0.045;

/// Quantidade de paterae sorteadas no recorte.
const IO_PATERA_COUNT: usize = 3;

/// Largura do halo avermelhado além da borda da patera, em fração do raio. Estilizado: os anéis
/// reais (ex.: Pele) são depósitos de pluma de centenas de km, bem maiores que o recorte.
const IO_HALO_WIDTH_FRACTION: f32 = 0.4;

/// Medidas das paterae de Io. As reais têm em média ~40 km de diâmetro e 0,5 a 2,5 km de
/// profundidade, maiores que o recorte de 22 km; aqui ficam paterae pequenas (2,4 a 6 km de
/// diâmetro) com centenas de metros de profundidade, para caberem várias no recorte. O paredão
/// chega a 1,7 de inclinação (~60 graus) e o talude ao pé dele começa em ~0,7 (~35 graus, perto
/// do ângulo de repouso de detritos). Valores de partida, a calibrar com o app rodando.
const IO_PATERA_SCALE: PateraScale = PateraScale {
    radius_min_m: 1200.0,
    radius_max_m: 3000.0,
    depth_min_m: 150.0,
    depth_max_m: 400.0,
    max_scarp_slope: 1.7,
    outline_amplitude_max: 0.08,
    outer_margin_fraction: IO_HALO_WIDTH_FRACTION,
};

/// Comprimento de onda da primeira oitava da irregularidade da borda das paterae, em metros.
/// As oitavas seguintes descem até ~45 m: reentrâncias e esporões do paredão.
const IO_PATERA_JITTER_WAVELENGTH_M: f32 = 360.0;

/// Oitavas da irregularidade da borda das paterae.
const IO_PATERA_JITTER_OCTAVES: u32 = 4;

/// Deslocamento máximo da borda das paterae pela irregularidade, em metros. Menor que a margem
/// do halo, então não muda o espaçamento entre paterae nem a distância à clareira de spawn.
const IO_PATERA_JITTER_AMPLITUDE_M: f32 = 90.0;

/// Comprimento de onda das cristas de pressão da crosta de lava no piso das paterae, em metros.
const IO_FLOOR_RIDGE_WAVELENGTH_M: f32 = 220.0;

/// Altura das cristas de pressão no piso das paterae, em metros.
const IO_FLOOR_RIDGE_AMPLITUDE_M: f32 = 9.0;

/// Piso da patera: lava silicática escura (RGBA linear).
const IO_PATERA_FLOOR_COLOR: [f32; 4] = [0.09, 0.08, 0.07, 1.0];

/// Paredão da patera: rocha exposta, mais escura e menos avermelhada que o halo (RGBA linear).
const IO_PATERA_WALL_COLOR: [f32; 4] = [0.26, 0.17, 0.11, 1.0];

/// Halo ao redor da patera: depósito avermelhado de enxofre de cadeia curta (RGBA linear).
const IO_HALO_COLOR: [f32; 4] = [0.58, 0.24, 0.13, 1.0];

/// Geada de SO2: manchas esbranquiçadas sobre as planícies (RGBA linear).
const IO_FROST_COLOR: [f32; 4] = [0.90, 0.90, 0.85, 1.0];

/// Comprimento de onda da primeira oitava da máscara de geada, em metros.
const IO_FROST_BASE_WAVELENGTH_M: f32 = 1800.0;

/// Oitavas da máscara de geada.
const IO_FROST_OCTAVES: u32 = 4;

/// Valor da máscara de geada (fbm normalizado, entre -1 e 1) onde a mancha começa a aparecer.
/// Calibrado pela cobertura medida (ver `io_frost_covers_a_minority_of_the_plains`).
const IO_FROST_MASK_START: f32 = 0.30;

/// Valor da máscara de geada a partir do qual a mancha fica com a cor plena.
const IO_FROST_MASK_END: f32 = 0.45;

/// Raio médio de Io, em metros, usado na queda da curvatura do anel de extensão.
const IO_RADIUS_M: f32 = 1_821_600.0;

/// Anel de extensão de Io: um bloco de 22 km em cada direção (até 33 km do centro, o que cabe no
/// `far` de 50 km da câmera vista do centro). 128 quads por bloco dá ~172 m entre vértices, o
/// bastante para a ondulação das planícies (comprimento de onda de ~4 km). Calibrável com o app
/// rodando.
const IO_FAR_FIELD: FarFieldConfig = FarFieldConfig {
    rings: 1,
    quads_per_tile: 128,
    moon_radius_m: IO_RADIUS_M,
    skirt_depth_factor: 0.35,
};

/// Mapa de cor de Io: 4096 texels nos 22 km do recorte dão ~5,4 m por texel, o espaçamento do
/// nível mais fino do LOD, em qualquer distância. Na cor por vértice, o nível mais grosso (~43 m)
/// errava a cor nas bordas das paterae em até 40 níveis de sRGB. Ocupa ~90 MB de GPU com os mips
/// e leva menos de 1 s para assar em vários núcleos.
const IO_COLOR_MAP: ColorMapConfig = ColorMapConfig {
    texels_per_side: 4096,
};

/// Monta a receita de geração de terreno para Io.
///
/// Não existe DTM público de Io com resolução útil para um recorte local, então o relevo é todo
/// procedural: planície de enxofre levemente ondulada com paterae escavadas (piso plano e
/// escarpa) e rugosidade fina por cima. A cor parte da inclinação das planícies, com manchas de
/// geada de SO2, e as paterae pintam piso escuro e halo avermelhado por cima.
pub(super) fn io_recipe(profile: &MoonProfile) -> TerrainRecipe {
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

    let terrain_with_clearing = io_relief(seed, calibration.base_frequency);

    // a inclinação vem só das planícies: a escarpa das paterae já tem a cor própria
    let slope_color = SlopeColorField {
        source: io_plains(seed, calibration.base_frequency),
        sample_step_m: IO_SLOPE_SAMPLE_STEP_M,
        flat_color: profile.terrain_base_color,
        steep_color: profile.terrain_steep_color,
        slope_start: IO_SLOPE_COLOR_START,
        slope_end: IO_SLOPE_COLOR_END,
    };

    let frost = ColorPatches {
        base: slope_color,
        mask: io_frost_mask(seed),
        patch_color: IO_FROST_COLOR,
        mask_start: IO_FROST_MASK_START,
        mask_end: IO_FROST_MASK_END,
    };

    let patera_color = PateraColorField {
        base: frost,
        paterae: io_paterae(seed),
        floor_color: IO_PATERA_FLOOR_COLOR,
        halo_color: IO_HALO_COLOR,
        wall_color: IO_PATERA_WALL_COLOR,
        halo_width_fraction: IO_HALO_WIDTH_FRACTION,
        outline_jitter: Some(io_patera_outline_jitter(seed)),
    };

    let color_with_clearing = FlattenColorNearOrigin {
        source: patera_color,
        flat_color: profile.terrain_base_color,
        flat_radius_m: SPAWN_CLEARING_RADIUS_M,
        blend_radius_m: SPAWN_CLEARING_BLEND_M,
    };

    let (appearance, material_properties) = appearance_and_material(profile);

    TerrainRecipe {
        height: arc(terrain_with_clearing),
        params: terrain_params,
        appearance,
        material_properties,
        color: Some(arc_color(color_with_clearing)),
        lod: chunked_lod_config(),
        far_field: Some(IO_FAR_FIELD),
        color_map: Some(IO_COLOR_MAP),
    }
}

/// Relevo completo de Io: planície com paterae escavadas, achatada na clareira de spawn, mais a
/// rugosidade fina por cima.
///
/// A rugosidade entra depois da escavação, para o piso de lava não ficar liso, e depois do
/// achatamento, para a clareira ser um platô no nível da planície ao redor e não um plano
/// perfeito: é a primeira coisa que se vê ao chegar em Io.
fn io_relief(
    seed: u32,
    base_frequency: f32,
) -> Add2<FlattenNearOrigin<CarvePaterae<PerlinFbm>>, PerlinFbm> {
    Add2 {
        a: FlattenNearOrigin::new(
            io_carved_plains(seed, base_frequency),
            SPAWN_CLEARING_RADIUS_M,
            SPAWN_CLEARING_BLEND_M,
        ),
        b: io_fine_detail(seed),
    }
}

/// Planícies com as paterae escavadas: borda irregular e cristas de pressão no piso.
fn io_carved_plains(seed: u32, base_frequency: f32) -> CarvePaterae<PerlinFbm> {
    CarvePaterae::new(io_plains(seed, base_frequency), io_paterae(seed))
        .with_outline_jitter(io_patera_outline_jitter(seed))
        .with_floor_relief(arc(io_floor_ridges(seed)))
}

/// Ondulação de grande escala das planícies de Io, centrada em zero.
fn io_plains(seed: u32, base_frequency: f32) -> PerlinFbm {
    PerlinFbm {
        perlin: Perlin::new(seed),
        freq: base_frequency,
        octaves: IO_PLAINS_OCTAVES,
        lacunarity: 2.0,
        gain: 0.5,
        amplitude: IO_PLAINS_AMPLITUDE_M,
    }
}

/// Paterae do recorte, fora do alcance da clareira de spawn (platô mais transição), com o halo
/// incluído, para não mudar a altura nem a cor do platô.
fn io_paterae(seed: u32) -> Vec<PateraSpec> {
    generate_patera_specs(
        seed,
        TERRAIN_SIZE_M * 0.5,
        IO_PATERA_COUNT,
        SPAWN_CLEARING_RADIUS_M + SPAWN_CLEARING_BLEND_M,
        &IO_PATERA_SCALE,
    )
}

/// Irregularidade da borda das paterae, com semente própria. O relevo e a cor chamam esta
/// mesma função, para o paredão e a cor dele coincidirem.
fn io_patera_outline_jitter(seed: u32) -> OutlineJitter {
    OutlineJitter {
        noise: arc(PerlinFbm {
            perlin: Perlin::new(seed.wrapping_add(3)),
            freq: 1.0 / IO_PATERA_JITTER_WAVELENGTH_M,
            octaves: IO_PATERA_JITTER_OCTAVES,
            lacunarity: 2.0,
            gain: 0.5,
            amplitude: 1.0,
        }),
        amplitude_m: IO_PATERA_JITTER_AMPLITUDE_M,
    }
}

/// Cristas de pressão da crosta de lava no piso das paterae, com semente própria.
fn io_floor_ridges(seed: u32) -> PerlinRidged {
    PerlinRidged {
        perlin: Perlin::new(seed.wrapping_add(4)),
        freq: 1.0 / IO_FLOOR_RIDGE_WAVELENGTH_M,
        octaves: 2,
        lacunarity: 2.0,
        gain: 0.5,
        amplitude: IO_FLOOR_RIDGE_AMPLITUDE_M,
        z_anisotropy: 1.0,
    }
}

/// Máscara de ruído das manchas de geada de SO2, com semente própria.
fn io_frost_mask(seed: u32) -> PerlinFbm {
    PerlinFbm {
        perlin: Perlin::new(seed.wrapping_add(2)),
        freq: 1.0 / IO_FROST_BASE_WAVELENGTH_M,
        octaves: IO_FROST_OCTAVES,
        lacunarity: 2.0,
        gain: 0.5,
        amplitude: 1.0,
    }
}

/// Rugosidade fina das planícies de Io, com semente própria para não repetir o padrão da
/// ondulação de grande escala.
fn io_fine_detail(seed: u32) -> PerlinFbm {
    PerlinFbm {
        perlin: Perlin::new(seed.wrapping_add(1)),
        freq: 1.0 / IO_DETAIL_BASE_WAVELENGTH_M,
        octaves: IO_DETAIL_OCTAVES,
        lacunarity: 2.0,
        gain: 2.0_f32.powf(-IO_DETAIL_HURST_EXPONENT),
        amplitude: IO_DETAIL_AMPLITUDE_M,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::height::HeightSource;
    use eigc_common::math::smoothstep;

    /// Perfil real de Io, para medir a receita com a frequência que o jogo usa.
    fn io_profile() -> MoonProfile {
        ron::de::from_str(include_str!("../../../../assets/moons/io.ron"))
            .expect("o io.ron deveria ser um MoonProfile válido")
    }

    /// Percentis 50, 95 e máximo da inclinação (subida/avanço) das planícies de Io, medida como
    /// `SlopeColorField` mede, numa grade que cobre o recorte.
    fn io_plains_slope_percentiles() -> (f32, f32, f32) {
        let profile = io_profile();
        let plains = io_plains(profile.terrain.seed, profile.terrain.base_frequency);
        let step = IO_SLOPE_SAMPLE_STEP_M;
        let mut slopes = Vec::new();
        for i in 0..150 {
            for j in 0..150 {
                let x = -10_500.0 + i as f32 * 140.3;
                let z = -10_500.0 + j as f32 * 139.7;
                let dx =
                    (plains.height_at(x + step, z) - plains.height_at(x - step, z)) / (2.0 * step);
                let dz =
                    (plains.height_at(x, z + step) - plains.height_at(x, z - step)) / (2.0 * step);
                slopes.push(dx.hypot(dz));
            }
        }
        slopes.sort_by(f32::total_cmp);
        let at = |q: f32| slopes[((slopes.len() - 1) as f32 * q) as usize];
        (at(0.5), at(0.95), at(1.0))
    }

    /// Testa que os limiares de cor por inclinação de Io acompanham a inclinação medida das
    /// planícies: a mediana fica abaixo do início (a maior parte é planície), o percentil 95
    /// passa do início (há encostas visíveis) e o máximo passa do fim (algumas saturam). Sem
    /// isso, a cor de encosta não aparece e o chão fica uniforme.
    #[test]
    fn io_slope_color_thresholds_follow_the_measured_plains_slope() {
        let (median, p95, max) = io_plains_slope_percentiles();
        println!("inclinação de Io: mediana {median}, p95 {p95}, máximo {max}");

        assert!(median < IO_SLOPE_COLOR_START, "mediana {median}");
        assert!(p95 > IO_SLOPE_COLOR_START, "p95 {p95}");
        assert!(max > IO_SLOPE_COLOR_END, "máximo {max}");
    }

    /// Testa que a semente do `io.ron` e outras sementes conseguem posicionar todas as paterae
    /// pedidas, sem descartar nenhuma por falta de espaço.
    #[test]
    fn every_requested_patera_fits_in_the_terrain() {
        for seed in [io_profile().terrain.seed, 1, 42, 99, 999, 31_337] {
            assert_eq!(io_paterae(seed).len(), IO_PATERA_COUNT, "seed {seed}");
        }
    }

    /// Testa que as paterae não mexem na clareira de spawn: o relevo escavado é idêntico ao das
    /// planícies em todo o alcance da clareira, e a altura do platô fica igual com e sem paterae.
    #[test]
    fn paterae_leave_the_spawn_clearing_plateau_unchanged() {
        let reach = SPAWN_CLEARING_RADIUS_M + SPAWN_CLEARING_BLEND_M;
        let base_frequency = io_profile().terrain.base_frequency;
        for seed in [1_u32, 42, 99, 501, 31_337] {
            let plains = io_plains(seed, base_frequency);
            let carved = io_carved_plains(seed, base_frequency);
            for ring in 0..=14 {
                let radius = reach * ring as f32 / 14.0;
                for step in 0..64 {
                    let angle = step as f32 / 64.0 * std::f32::consts::TAU;
                    let (x, z) = (angle.cos() * radius, angle.sin() * radius);
                    assert_eq!(
                        carved.height_at(x, z),
                        plains.height_at(x, z),
                        "seed {seed}: patera dentro da clareira em ({x}, {z})"
                    );
                }
            }

            let with_paterae =
                FlattenNearOrigin::new(carved, SPAWN_CLEARING_RADIUS_M, SPAWN_CLEARING_BLEND_M);
            let without_paterae =
                FlattenNearOrigin::new(plains, SPAWN_CLEARING_RADIUS_M, SPAWN_CLEARING_BLEND_M);
            assert_eq!(with_paterae.flat_height, without_paterae.flat_height);
        }
    }

    /// Testa que a clareira de spawn de Io é um platô com rugosidade fina, e não um plano
    /// perfeito: dentro do raio plano, a altura é exatamente o platô mais a rugosidade, e varia
    /// de um ponto a outro.
    #[test]
    fn io_spawn_clearing_keeps_the_fine_roughness_over_the_plateau() {
        let profile = io_profile();
        let seed = profile.terrain.seed;
        let recipe = io_recipe(&profile);
        let plateau = FlattenNearOrigin::new(
            io_carved_plains(seed, profile.terrain.base_frequency),
            SPAWN_CLEARING_RADIUS_M,
            SPAWN_CLEARING_BLEND_M,
        )
        .flat_height;
        let detail = io_fine_detail(seed);

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

    /// Diferença rms de altura entre pontos a `lag_m` de distância (nos dois eixos), numa grade
    /// que cobre o recorte.
    fn rms_height_difference(height: &dyn HeightSource, lag_m: f32) -> f32 {
        let mut sum = 0.0_f64;
        let mut count = 0;
        for i in 0..150 {
            for j in 0..150 {
                let x = -10_500.0 + i as f32 * 140.3;
                let z = -10_500.0 + j as f32 * 139.7;
                let base = height.height_at(x, z);
                for (dx, dz) in [(lag_m, 0.0), (0.0, lag_m)] {
                    let difference = height.height_at(x + dx, z + dz) - base;
                    sum += f64::from(difference * difference);
                    count += 1;
                }
            }
        }
        (sum / f64::from(count)).sqrt() as f32
    }

    /// Testa que as planícies de Io (ondulação mais rugosidade fina, sem paterae) têm rugosidade
    /// visível na escala de quem anda: diferença rms de ~2 m entre pontos a 29 m, a mesma ordem
    /// do relevo de Europa, e inclinação rms acima de 0,08 (~4,6 graus) entre vértices do nível 0
    /// da malha. Antes eram 0,7 m e 0,027 (~1,5 grau), e o chão parecia liso de perto.
    #[test]
    fn io_plains_roughness_reaches_the_target_at_small_scale() {
        let profile = io_profile();
        let seed = profile.terrain.seed;
        let plains = Add2 {
            a: io_plains(seed, profile.terrain.base_frequency),
            b: io_fine_detail(seed),
        };

        let at_29_m = rms_height_difference(&plains, 29.0);
        let mesh_spacing_m = 5.4;
        let slope_at_mesh = rms_height_difference(&plains, mesh_spacing_m) / mesh_spacing_m;
        println!("rugosidade de Io: rms a 29 m {at_29_m}, inclinação rms na malha {slope_at_mesh}");

        assert!((1.5..2.5).contains(&at_29_m), "rms a 29 m: {at_29_m}");
        assert!(slope_at_mesh > 0.08, "inclinação rms na malha: {slope_at_mesh}");
    }

    /// Testa que a geada de SO2 cobre uma minoria das planícies (entre 5% e 25% do recorte com
    /// mais da metade da cor de geada): visível, mas sem trocar o amarelo do enxofre pelo branco.
    #[test]
    fn io_frost_covers_a_minority_of_the_plains() {
        let mask = io_frost_mask(io_profile().terrain.seed);
        let mut covered = 0;
        let total = 150 * 150;
        for i in 0..150 {
            for j in 0..150 {
                let x = -10_500.0 + i as f32 * 140.3;
                let z = -10_500.0 + j as f32 * 139.7;
                let weight =
                    smoothstep(IO_FROST_MASK_START, IO_FROST_MASK_END, mask.height_at(x, z));
                if weight > 0.5 {
                    covered += 1;
                }
            }
        }
        let fraction = covered as f32 / total as f32;
        println!("cobertura de geada: {fraction}");
        assert!((0.05..0.25).contains(&fraction), "cobertura {fraction}");
    }

    /// Testa que a receita pinta as paterae: o centro de cada uma tem a cor escura do piso e
    /// fica abaixo da planície ao redor, e logo fora da borda o halo deixa a cor avermelhada.
    #[test]
    fn io_recipe_paints_dark_floors_and_reddish_halos_on_its_paterae() {
        let profile = io_profile();
        let recipe = io_recipe(&profile);
        let color = recipe.color.expect("Io deveria ter cor por vértice");

        for spec in io_paterae(profile.terrain.seed) {
            let center = spec.center;
            let floor = color.color_at(center.x, center.y);
            for (got, expected) in floor.iter().zip(IO_PATERA_FLOOR_COLOR) {
                assert!((got - expected).abs() < 1e-5, "piso em {center}: {floor:?}");
            }

            let rim = spec.rim_radius_at(0.0);
            let floor_height = recipe.height.height_at(center.x, center.y);
            let outside = center + Vec2::X * rim * (1.0 + IO_HALO_WIDTH_FRACTION * 1.2);
            assert!(
                floor_height
                    < recipe.height.height_at(outside.x, outside.y)
                        - 0.5 * IO_PATERA_SCALE.depth_min_m,
                "o piso em {center} deveria ficar bem abaixo da planície"
            );

            let halo = center + Vec2::X * rim * (1.0 + IO_HALO_WIDTH_FRACTION * 0.1);
            let [r, g, b, _] = color.color_at(halo.x, halo.y);
            assert!(
                r > 1.5 * g && r > 1.5 * b,
                "halo em {halo} deveria ser avermelhado: {r} {g} {b}"
            );
        }
    }
}
