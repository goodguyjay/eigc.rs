//! Receita de terreno de Europa: DTM real com lineae e rugosidade fractal por cima.

use super::{
    SPAWN_CLEARING_BLEND_M, SPAWN_CLEARING_RADIUS_M, TERRAIN_RES, TERRAIN_SIZE_M, TerrainRecipe,
    VERTICAL_EXAGGERATION, appearance_and_material, chunked_lod_config,
};
use crate::height::comb::{Add2, Bias, FlattenColorNearOrigin, FlattenNearOrigin, Scale};
use crate::height::heightmap::HeightmapHeight;
use crate::height::linea::{LineaField, LineaScale, generate_linea_specs};
use crate::height::noise::PerlinFbm;
use crate::height::slope::SlopeColorField;
use crate::height::{HeightSource, arc, arc_color};
use crate::params::TerrainParams;
use bevy::prelude::Vec2;
use eigc_moons::profile::MoonProfile;
use noise::Perlin;

/// PNG de 16 bits do DTM real de Europa (USGS/Galileo, sítio Rhadamanthys), gerado por
/// `tools/dtm_to_heightmap.py`. Embutido no binário para `build_recipe`.
const EUROPA_HEIGHTMAP_PNG: &[u8] =
    include_bytes!("../../../../assets/terrain/europa/europa_rhadamanthys_patch_heightmap.png");

/// Metadados RON do heightmap embutido (escala em metros e faixa de elevação).
const EUROPA_HEIGHTMAP_RON: &str =
    include_str!("../../../../assets/terrain/europa/europa_rhadamanthys_patch_heightmap.ron");

/// Distância correspondente a 1 pixel do DTM, em metros.
const DTM_PIXEL_M: f32 = 227.65;

/// Diferença rms de altura do DTM entre pontos a 1 pixel de distância, em metros (13,0 medido
/// direto, 13,6 pelo ajuste da lei de potência; fica no meio).
const DTM_ROUGHNESS_AT_PIXEL_M: f32 = 13.2;

/// Comprimento de onda da primeira oitava do detalhe sintético, em metros: 1 pixel do DTM. A
/// rugosidade do DTM já segue a lei de potência abaixo até ~190 m de distância; começar o detalhe
/// em escalas maiores somaria rugosidade que o DTM já tem.
const DETAIL_BASE_WAVELENGTH_M: f32 = DTM_PIXEL_M;

/// Oitavas do detalhe sintético: de ~228 m a ~28 m de comprimento de onda. Abaixo disso o nível 0
/// da malha (~5,4 m entre vértices) teria poucos vértices por comprimento de onda.
const DETAIL_OCTAVES: u32 = 4;

/// Expoente de Hurst da rugosidade do DTM: a diferença rms de altura entre dois pontos cresce com
/// a distância elevada a `H`. Medido no recorte de Rhadamanthys (ajuste entre 1 e 12 pixels de
/// distância, `H` entre 0,85 e 0,89), ver `tools/dtm_roughness.py`.
const DTM_HURST_EXPONENT: f32 = 0.85;

/// Quantidade de lineae do recorte, dentro da faixa considerada de 40 a 80. Decisão da autora, a
/// recalibrar após inspeção visual.
const LINEA_COUNT: usize = 60;

/// Medidas das lineae de Europa como detalhe entre o DTM e a rugosidade fractal: crista de 100 a
/// 400 m, onde o DTM tem pouca energia (desvio padrão de ~2 m entre 455 m e 1 km de comprimento
/// de onda) e o fractal ainda não chega (primeira oitava em 228 m). Decisão da autora, a
/// recalibrar após inspeção visual.
///
/// A altura segue a rugosidade rms do DTM numa distância igual à largura da crista (mesma lei de
/// potência do detalhe fractal): ~6,4 m para 100 m de largura, ~21 m para 400 m. Cada linea
/// sorteia de 75% a 100% da altura dessa regra (`height_fraction_min`), de ~5 a ~21 m. Assim a
/// crista tem o tamanho do relevo que o DTM já tem nessa escala e não compete com ele. O teto de
/// altura/largura de 0,065 dá inclinação máxima de ~0,20 (π × 0,065), perto do p95 do DTM (0,17).
///
/// As famílias de orientação 1 e 2 seguem herdadas a ±50 graus da direção medida
/// (`EUROPA_MEASURED_RIDGE_DIRECTION`). O DTM sustenta pouco essas famílias (~5% do peso do
/// gradiente em cada uma); o ângulo fica como decisão da autora, a recalibrar após inspeção
/// visual.
const EUROPA_LINEA_SCALE: LineaScale = LineaScale {
    ridge_width_min_m: 100.0,
    regular_width_max_m: 300.0,
    ridge_width_max_m: 400.0,
    valley_to_ridge_min: 0.5,
    valley_to_ridge_max: 1.0,
    length_min_m: 1500.0,
    regular_length_max_m: 4500.0,
    length_max_m: 6000.0,
    height_at_reference_width_m: DTM_ROUGHNESS_AT_PIXEL_M,
    reference_width_m: DTM_PIXEL_M,
    height_width_exponent: DTM_HURST_EXPONENT,
    height_fraction_min: 0.75,
    max_slope_ratio: 0.065,
    family_offset_rad: 0.8727,
};

/// Direção dominante das cristas no DTM de Rhadamanthys, no referencial do terreno (x = coluna do
/// raster, z = linha). Usada no lugar de `feature_direction` do perfil, que é (0.8, 0.2) ou +14
/// graus e nunca foi conferida contra o DTM.
///
/// Medida pelo tensor de estrutura do gradiente (médias de gx², gz² e gx·gz no recorte sem o
/// plano médio; as cristas correm na perpendicular do gradiente dominante): -24,5 graus a partir
/// de +x girando para +z com coerência 0,46, e -23,3 graus com coerência 0,27 olhando só as
/// escalas abaixo de ~700 m. Incerteza alta: amostra de 97x97 pixels com poucas cristas grandes,
/// e as cristas menores que o DTM não resolve podem ter outra orientação. Decisão da autora, a
/// recalibrar após inspeção visual; o `europa.ron` não foi alterado.
const EUROPA_MEASURED_RIDGE_DIRECTION: Vec2 = Vec2::new(0.91, -0.41);

/// Amplitude do detalhe sintético (soma normalizada das oitavas), em metros. Calibrada para que
/// a rugosidade total (DTM + detalhe) siga a lei de potência medida do DTM (ver o teste
/// `fine_detail_continues_the_dtm_roughness_power_law`).
const DETAIL_AMPLITUDE_M: f32 = 10.0;

/// Distância, em metros, entre as amostras usadas para estimar a inclinação do DTM na cor.
/// Da ordem de meio pixel do DTM (227,65 m).
const SLOPE_SAMPLE_STEP_M: f32 = 100.0;

/// Inclinação do DTM (subida/avanço) abaixo da qual o terreno recebe só a cor de gelo plano.
/// A inclinação média do recorte é ~0,05 e o percentil 95 ~0,16.
const SLOPE_COLOR_START: f32 = 0.06;

/// Inclinação do DTM (subida/avanço) a partir da qual o terreno recebe totalmente a cor de vale.
const SLOPE_COLOR_END: f32 = 0.20;

/// Monta a receita de geração de terreno para Europa.
/// A forma macro vem de um DTM real (USGS/Galileo) recentrado em zero e exagerado
/// verticalmente, por cima entram lineae de centenas de metros e uma rugosidade fina procedural.
/// Cor por inclinação do DTM (as lineae não entram na cor).
pub(super) fn europa_recipe(profile: &MoonProfile) -> TerrainRecipe {
    let calibration = &profile.terrain;

    let seed = calibration.seed;
    let base_frequency = calibration.base_frequency;

    // em Europa a direção das lineae vem do DTM, não de `calibration.feature_direction`
    let ridge_direction = EUROPA_MEASURED_RIDGE_DIRECTION.normalize();

    let heightmap = HeightmapHeight::from_png_and_ron(EUROPA_HEIGHTMAP_PNG, EUROPA_HEIGHTMAP_RON)
        .expect("o heightmap de Europa embutido deveria ser valido");

    // O DTM guarda elevação absoluta (de -514 a -64 m). Subtrair a elevação no centro mantém o
    // terreno em torno de y=0, onde câmera, céu e clareira esperam, e faz o exagero vertical
    // crescer a partir desse ponto em vez de a partir do datum.
    let reference_elevation_m = heightmap.height_at(0.0, 0.0);

    let terrain_params = TerrainParams {
        size: TERRAIN_SIZE_M,
        res: TERRAIN_RES,
        freq: base_frequency,
        line_dir: ridge_direction,
        seed,
        vertical_exaggeration: VERTICAL_EXAGGERATION,
    };

    let slope_source = heightmap.clone();

    let real_relief = Scale {
        s: Bias {
            s: heightmap,
            bias: -reference_elevation_m,
        },
        scale: terrain_params.vertical_exaggeration,
    };

    // lineae em metros absolutos, sem o exagero vertical: a altura delas já é calibrada contra a
    // rugosidade medida do DTM.
    let relief_with_lineae = Add2 {
        a: real_relief,
        b: europa_lineae(seed, ridge_direction),
    };

    let hybrid_terrain = Add2 {
        a: relief_with_lineae,
        b: europa_fine_detail(seed),
    };

    // clareira plana na origem: garante um ponto de spawn legível e sem relevo caótico, com
    // transição suave até o terreno completo (ver SPAWN_CLEARING_RADIUS_M/BLEND_M). A altura do
    // platô é derivada do relevo real na borda do blend (média ao redor do perímetro).
    let terrain_with_clearing = FlattenNearOrigin::new(
        hybrid_terrain,
        SPAWN_CLEARING_RADIUS_M,
        SPAWN_CLEARING_BLEND_M,
    );

    let slope_color = SlopeColorField {
        source: slope_source,
        sample_step_m: SLOPE_SAMPLE_STEP_M,
        flat_color: profile.terrain_base_color,
        steep_color: profile.terrain_steep_color,
        slope_start: SLOPE_COLOR_START,
        slope_end: SLOPE_COLOR_END,
    };

    // cor da clareira acompanha o mesmo raio/blend da altura: gelo claro e uniforme, não a cor
    // de vale que uma encosta próxima da origem poderia produzir.
    let color_with_clearing = FlattenColorNearOrigin {
        source: slope_color,
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

/// Detalhe sintético que estende o espectro de rugosidade do DTM para as escalas menores que 2
/// pixels. Cada oitava tem metade do comprimento de onda e `2^-H` da amplitude da anterior, a
/// mesma lei de potência medida no DTM.
fn europa_fine_detail(seed: u32) -> PerlinFbm {
    PerlinFbm {
        perlin: Perlin::new(seed),
        freq: 1.0 / DETAIL_BASE_WAVELENGTH_M,
        octaves: DETAIL_OCTAVES,
        lacunarity: 2.0,
        gain: 2.0_f32.powf(-DTM_HURST_EXPONENT),
        amplitude: DETAIL_AMPLITUDE_M,
    }
}

/// Lineae de Europa: cristas duplas de centenas de metros somadas ao DTM, fora do alcance da
/// clareira de spawn (platô mais transição), para não mudar a altura do platô.
fn europa_lineae(seed: u32, feature_direction: Vec2) -> LineaField {
    LineaField {
        specs: generate_linea_specs(
            seed,
            feature_direction,
            TERRAIN_SIZE_M * 0.5,
            LINEA_COUNT,
            SPAWN_CLEARING_RADIUS_M + SPAWN_CLEARING_BLEND_M,
            &EUROPA_LINEA_SCALE,
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dtm() -> HeightmapHeight {
        HeightmapHeight::from_png_and_ron(EUROPA_HEIGHTMAP_PNG, EUROPA_HEIGHTMAP_RON)
            .expect("o heightmap de Europa embutido deveria ser valido")
    }

    fn europa_direction() -> Vec2 {
        EUROPA_MEASURED_RIDGE_DIRECTION.normalize()
    }

    /// Diferença rms de altura entre pontos separados por `lag_m`, nos eixos x e z, numa grade
    /// que evita a borda do recorte.
    fn rms_difference(source: &dyn HeightSource, lag_m: f32) -> f32 {
        let (mut sum, mut count) = (0.0_f64, 0_u32);
        for i in 0..120 {
            for j in 0..120 {
                let x = -8000.0 + i as f32 * 131.7;
                let z = -8000.0 + j as f32 * 129.3;
                let h = source.height_at(x, z);
                for other in [
                    source.height_at(x + lag_m, z),
                    source.height_at(x, z + lag_m),
                ] {
                    sum += f64::from(other - h).powi(2);
                    count += 1;
                }
            }
        }
        (sum / f64::from(count)).sqrt() as f32
    }

    /// Testa que o detalhe sintético (lineae e rugosidade fractal) continua a lei de potência de
    /// rugosidade do DTM: entre 24 m e 900 m a diferença rms de altura total fica a até 15% da
    /// extrapolada de `H` e da rugosidade a 1 pixel, ou seja, as lineae não competem com o
    /// relevo do DTM. O DTM sozinho fica claramente abaixo da lei nas distâncias curtas (é o
    /// detalhe que preenche a lacuna).
    #[test]
    fn fine_detail_continues_the_dtm_roughness_power_law() {
        let dtm_only = dtm();
        let hybrid = Add2 {
            a: Add2 {
                a: dtm(),
                b: europa_lineae(204, europa_direction()),
            },
            b: europa_fine_detail(204),
        };

        let rows: Vec<(f32, f32, f32)> = [24.0_f32, 48.0, 96.0, 192.0, 384.0, 768.0, 900.0]
            .into_iter()
            .map(|lag_m| {
                let target =
                    DTM_ROUGHNESS_AT_PIXEL_M * (lag_m / DTM_PIXEL_M).powf(DTM_HURST_EXPONENT);
                (lag_m, rms_difference(&hybrid, lag_m), target)
            })
            .collect();
        for &(lag_m, total, target) in &rows {
            assert!(
                (total / target - 1.0).abs() < 0.15,
                "a {lag_m} m: rms {total}, alvo {target} (distância, rms, alvo: {rows:?})"
            );
        }

        assert!(rms_difference(&dtm_only, 24.0) < 0.85 * rms_difference(&hybrid, 24.0));
    }

    /// Testa que as lineae não mexem na clareira de spawn: valem exatamente zero em todo o
    /// alcance da clareira (platô mais transição), e a altura do platô, tirada da borda da
    /// transição, é idêntica com e sem lineae, para várias sementes.
    #[test]
    fn lineae_leave_the_spawn_clearing_plateau_unchanged() {
        let reach = SPAWN_CLEARING_RADIUS_M + SPAWN_CLEARING_BLEND_M;
        for seed in [1_u32, 42, 204, 999, 31_337] {
            let lineae = europa_lineae(seed, europa_direction());
            for ring in 0..=14 {
                let radius = reach * ring as f32 / 14.0;
                for step in 0..64 {
                    let angle = step as f32 / 64.0 * std::f32::consts::TAU;
                    let (x, z) = (angle.cos() * radius, angle.sin() * radius);
                    assert_eq!(
                        lineae.height_at(x, z),
                        0.0,
                        "seed {seed}: linea dentro da clareira em ({x}, {z})"
                    );
                }
            }

            let with_lineae = FlattenNearOrigin::new(
                Add2 {
                    a: Add2 {
                        a: dtm(),
                        b: lineae,
                    },
                    b: europa_fine_detail(seed),
                },
                SPAWN_CLEARING_RADIUS_M,
                SPAWN_CLEARING_BLEND_M,
            );
            let without_lineae = FlattenNearOrigin::new(
                Add2 {
                    a: dtm(),
                    b: europa_fine_detail(seed),
                },
                SPAWN_CLEARING_RADIUS_M,
                SPAWN_CLEARING_BLEND_M,
            );
            assert_eq!(with_lineae.flat_height, without_lineae.flat_height);
        }
    }
}
