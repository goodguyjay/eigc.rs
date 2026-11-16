//! Receita de terreno de Io: relevo todo procedural, sem DTM.

use super::{
    SPAWN_CLEARING_BLEND_M, SPAWN_CLEARING_RADIUS_M, TERRAIN_RES, TERRAIN_SIZE_M, TerrainRecipe,
    VERTICAL_EXAGGERATION, appearance_and_material, chunked_lod_config,
};
use crate::height::comb::{Add2, FlattenColorNearOrigin, FlattenNearOrigin};
use crate::height::noise::PerlinFbm;
use crate::height::patches::ColorPatches;
use crate::height::patera::{
    CarvePaterae, PateraColorField, PateraScale, PateraSpec, generate_patera_specs,
};
use crate::height::slope::SlopeColorField;
use crate::height::{arc, arc_color};
use crate::params::TerrainParams;
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

/// Oitavas da rugosidade fina de Io: de ~230 m a ~29 m, mesmo limite do detalhe de Europa (o
/// nível 0 da malha, ~5,4 m entre vértices, teria poucos vértices por comprimento de onda).
const IO_DETAIL_OCTAVES: u32 = 4;

/// Amplitude da rugosidade fina de Io, em metros. Valor de partida, não medido.
const IO_DETAIL_AMPLITUDE_M: f32 = 4.0;

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
/// diâmetro) com centenas de metros de profundidade, para caberem várias no recorte. A escarpa
/// chega a 0,8 de inclinação (~39 graus). Valores de partida, a calibrar com o app rodando.
const IO_PATERA_SCALE: PateraScale = PateraScale {
    radius_min_m: 1200.0,
    radius_max_m: 3000.0,
    depth_min_m: 150.0,
    depth_max_m: 400.0,
    max_scarp_slope: 0.8,
    outline_amplitude_max: 0.08,
    outer_margin_fraction: IO_HALO_WIDTH_FRACTION,
};

/// Piso da patera: lava silicática escura (RGBA linear).
const IO_PATERA_FLOOR_COLOR: [f32; 4] = [0.09, 0.08, 0.07, 1.0];

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
    };

    let paterae = io_paterae(seed);

    // a rugosidade fina entra depois da escavação para o piso de lava não ficar liso demais
    let terrain = Add2 {
        a: CarvePaterae::new(io_plains(seed, calibration.base_frequency), paterae.clone()),
        b: io_fine_detail(seed),
    };

    let terrain_with_clearing =
        FlattenNearOrigin::new(terrain, SPAWN_CLEARING_RADIUS_M, SPAWN_CLEARING_BLEND_M);

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
        paterae,
        floor_color: IO_PATERA_FLOOR_COLOR,
        halo_color: IO_HALO_COLOR,
        halo_width_fraction: IO_HALO_WIDTH_FRACTION,
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
    }
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
        gain: 0.5,
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
            let carved = CarvePaterae::new(io_plains(seed, base_frequency), io_paterae(seed));
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
