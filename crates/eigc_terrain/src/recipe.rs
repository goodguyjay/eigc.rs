//! Monta a recipe de geração de terreno para cada lua suportada.
//! Esse módulo é o único lugar onde MoonId decide qual composição de HeightSource e quais
//! parâmetros visuais correspondem a uma lua específica.

use crate::height::comb::{Add2, Bias, FlattenColorNearOrigin, FlattenNearOrigin, Scale};
use crate::height::heightmap::HeightmapHeight;
use crate::height::linea::{LineaField, LineaScale, generate_linea_specs};
use crate::height::noise::PerlinFbm;
use crate::height::tint::TerrainTintField;
use crate::height::{ColorFn, HeightFn, HeightSource, arc, arc_color};
use crate::lod::TerrainLodConfig;
use crate::params::TerrainParams;
use crate::pipeline::TerrainAppearance;
use crate::systems::TerrainMaterialProperties;
use bevy::prelude::{Color, Vec2};
use eigc_moons::profile::{MoonId, MoonProfile};
use noise::Perlin;

/// PNG de 16 bits do DTM real de Europa (USGS/Galileo, sítio Rhadamanthys), gerado por
/// `tools/dtm_to_heightmap.py`. Embutido no binário para `build_recipe`.
const EUROPA_HEIGHTMAP_PNG: &[u8] =
    include_bytes!("../../../assets/terrain/europa/europa_rhadamanthys_patch_heightmap.png");

/// Metadados RON do heightmap embutido (escala em metros e faixa de elevação).
const EUROPA_HEIGHTMAP_RON: &str =
    include_str!("../../../assets/terrain/europa/europa_rhadamanthys_patch_heightmap.ron");

/// Exagero vertical aplicado ao relevo real (ver `TerrainParams::vertical_exaggeration`).
/// `1.0` mantém a elevação medida no DTM.
const VERTICAL_EXAGGERATION: f32 = 1.0;

/// Tamanho do recorte, em metros. Casa com o DTM (~22082 m); o excedente de ~41 m por lado do DTM
/// fica fora do terreno.
const TERRAIN_SIZE_M: f32 = 22000.0;

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

/// Desvio, em metros, do borrão do DTM que define o "entorno" na altura relativa (altura do DTM
/// menos o DTM borrado). Com 1 km, a altura relativa do recorte tem desvio de ~25 m (percentis
/// 5 e 95 em -47 e +42 m).
const RELATIVE_HEIGHT_BLUR_SIGMA_M: f32 = 1000.0;

/// Quantos metros abaixo do entorno o tingimento por altura começa (~percentil 30 do recorte).
const RELATIVE_HEIGHT_TINT_START_M: f32 = 10.0;

/// Quantos metros abaixo do entorno o tingimento por altura chega ao máximo (~percentil 5).
const RELATIVE_HEIGHT_TINT_FULL_M: f32 = 45.0;

/// Peso máximo que a altura relativa sozinha dá ao tingimento: com 0,9 um vale fundo e plano fica
/// quase com `terrain_valley_color` (e chega a ela onde a mancha passa de 1,1).
const RELATIVE_HEIGHT_TINT_SCALE: f32 = 0.9;

/// Comprimento de onda da primeira oitava das manchas pela posição, em metros.
const MOTTLE_WAVELENGTH_M: f32 = 6000.0;

/// Oitavas das manchas: com `MOTTLE_LACUNARITY` 3, comprimentos de onda de 6 km e 2 km.
const MOTTLE_OCTAVES: u32 = 2;

/// Multiplicador de frequência entre as oitavas das manchas.
const MOTTLE_LACUNARITY: f32 = 3.0;

/// Fator de mancha mínimo aplicado ao peso de tingimento (manchas mais claras).
const MOTTLE_MIN: f32 = 0.5;

/// Fator de mancha máximo aplicado ao peso de tingimento (manchas mais tingidas).
const MOTTLE_MAX: f32 = 1.4;

/// Misturado à semente da lua para o ruído das manchas não repetir o do detalhe fino.
const MOTTLE_SEED_SALT: u32 = 0x5EED_C010;

/// Raio da clareira plana ao redor da origem (diâmetro ~300m), onde o jogador/câmera aparece.
const SPAWN_CLEARING_RADIUS_M: f32 = 150.0;

/// Distância adicional, além de `SPAWN_CLEARING_RADIUS_M`, em que o terreno transiciona
/// suavemente da clareira plana até os parâmetros completos da receita.
const SPAWN_CLEARING_BLEND_M: f32 = 200.0;

/// Agrupa os produtos de uma receita de terreno: a função de altura composta, os parâmetros
/// de malha/mundo, a aparência visual e, opcionalmente, uma fonte de cor por vértice.
pub struct TerrainRecipe {
    /// Altura do terreno
    pub height: HeightFn,
    /// Parâmetros de malha/mundo do terreno
    pub params: TerrainParams,
    /// Aparência visual do terreno
    pub appearance: TerrainAppearance,
    /// Propriedades do material do terreno
    pub material_properties: TerrainMaterialProperties,
    /// Fonte de cor por vértice, quando a receita tinge o terreno (em Europa, por inclinação,
    /// altura relativa ao entorno e manchas pela posição). `None`
    /// significa que o terreno usa só a cor flat de `appearance.base_color`.
    pub color: Option<ColorFn>,
    /// Configuração do LOD por chunks do terreno.
    pub lod: TerrainLodConfig,
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
/// A forma macro vem de um DTM real (USGS/Galileo) recentrado em zero e exagerado
/// verticalmente, por cima entram lineae de centenas de metros e uma rugosidade fina procedural.
/// Cor por inclinação e altura relativa do DTM, com manchas pela posição
/// (`europa_terrain_color`); as lineae não entram na cor.
fn europa_recipe(profile: &MoonProfile) -> TerrainRecipe {
    // Resolução da malha monolítica legada (vértices por lado). O terreno em jogo usa chunks
    // com LOD (ver `lod` abaixo), então isto só alimenta `TerrainParams.res` (ver BACKLOG.md).
    const TERRAIN_RES: u32 = 1536;

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
        amp: calibration.vertical_amplitude_meters,
        freq: base_frequency,
        line_dir: ridge_direction,
        seed,
        vertical_exaggeration: VERTICAL_EXAGGERATION,
    };

    let terrain_color = europa_terrain_color(
        &heightmap,
        seed,
        profile.terrain_base_color,
        profile.terrain_valley_color,
    );

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

    let terrain_appearance = TerrainAppearance {
        base_color: color_from_linear_rgba(profile.terrain_base_color),
        display_name: profile.display_name.clone(),
    };

    let material_properties = TerrainMaterialProperties {
        perceptual_roughness: calibration.perceptual_roughness,
        reflectance: calibration.reflectance,
    };

    // 64x64 chunks de 343,75 m. O nível 0 tem espaçamento de ~5,4 m (melhor que os 14,3 m da
    // malha monolítica) e o mais grosso ~43 m, menor que a largura das cristas das lineae
    // (100 a 400 m).
    // O nível de cada chunk sai do erro geométrico medido (ver `lod_error.rs`) projetado na tela:
    // `max_screen_error_px` menor = mais detalhe de mais longe. `min_error_fraction` é o piso
    // para chunks lisos (e para a cor por vértice, que também perde resolução ao engrossar).
    // Valores calibráveis: medir com o app rodando antes de mexer.
    let lod = TerrainLodConfig {
        chunks_per_side: 64,
        quads_per_chunk: [64, 32, 16, 8],
        max_screen_error_px: 1.5,
        min_error_fraction: 0.2,
        hysteresis_fraction: 0.1,
        skirt_depth_factor: 0.35,
        in_flight_vertex_budget: 30_000,
        max_error_tasks_in_flight: 16,
    };

    TerrainRecipe {
        height: arc(terrain_with_clearing),
        params: terrain_params,
        appearance: terrain_appearance,
        material_properties,
        color: Some(arc_color(terrain_color)),
        lod,
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

/// Ruído de baixa frequência, em `[-1, 1]`, das manchas de tingimento de Europa pela posição.
fn europa_mottle_noise(seed: u32) -> PerlinFbm {
    PerlinFbm {
        perlin: Perlin::new(seed ^ MOTTLE_SEED_SALT),
        freq: 1.0 / MOTTLE_WAVELENGTH_M,
        octaves: MOTTLE_OCTAVES,
        lacunarity: MOTTLE_LACUNARITY,
        gain: 0.5,
        amplitude: 1.0,
    }
}

/// Cor por vértice de Europa: tingimento por inclinação e por altura relativa ao entorno (DTM
/// menos o DTM borrado uma vez aqui), modulado por manchas pela posição, com a clareira de spawn
/// em `flat_color` (gelo claro e uniforme, não a cor de vale que uma encosta próxima da origem
/// poderia produzir).
fn europa_terrain_color(
    heightmap: &HeightmapHeight,
    seed: u32,
    flat_color: [f32; 4],
    tinted_color: [f32; 4],
) -> FlattenColorNearOrigin<TerrainTintField<HeightmapHeight, HeightmapHeight, PerlinFbm>> {
    let tint = TerrainTintField {
        source: heightmap.clone(),
        smoothed_source: heightmap.blurred(RELATIVE_HEIGHT_BLUR_SIGMA_M),
        mottle_noise: europa_mottle_noise(seed),
        sample_step_m: SLOPE_SAMPLE_STEP_M,
        slope_start: SLOPE_COLOR_START,
        slope_end: SLOPE_COLOR_END,
        depth_start_m: RELATIVE_HEIGHT_TINT_START_M,
        depth_full_m: RELATIVE_HEIGHT_TINT_FULL_M,
        height_weight_scale: RELATIVE_HEIGHT_TINT_SCALE,
        mottle_min: MOTTLE_MIN,
        mottle_max: MOTTLE_MAX,
        flat_color,
        tinted_color,
    };

    FlattenColorNearOrigin {
        source: tint,
        flat_color,
        flat_radius_m: SPAWN_CLEARING_RADIUS_M,
        blend_radius_m: SPAWN_CLEARING_BLEND_M,
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

/// Converte um array [r, g, b, a] em componentes lineares (formato salvo no .ron) para o tipo Color do bevy
fn color_from_linear_rgba(components: [f32; 4]) -> Color {
    Color::srgba(components[0], components[1], components[2], components[3])
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::height::ColorSource;

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

    const ICE: [f32; 4] = [1.0, 1.0, 1.0, 1.0];
    const VALLEY: [f32; 4] = [0.55, 0.48, 0.42, 1.0];

    /// Pontos de uma grade que cobre o recorte de Europa sem a borda.
    fn map_grid(steps: usize) -> impl Iterator<Item = (f32, f32)> {
        let half = 10_000.0;
        (0..steps).flat_map(move |i| {
            (0..steps).map(move |j| {
                let span = 2.0 * half / (steps - 1) as f32;
                (-half + i as f32 * span, -half + j as f32 * span)
            })
        })
    }

    /// Testa que o ruído das manchas é o mesmo para a mesma semente, muda com a semente, fica em
    /// `[-1, 1]` e varia de verdade pelo mapa (a conversão para `[MOTTLE_MIN, MOTTLE_MAX]` é
    /// testada em `height::tint`).
    #[test]
    fn mottle_noise_is_deterministic_per_seed_and_stays_in_range() {
        let noise = europa_mottle_noise(204);
        let same = europa_mottle_noise(204);
        let other = europa_mottle_noise(205);

        let values: Vec<f32> = map_grid(60).map(|(x, z)| noise.height_at(x, z)).collect();
        let repeated: Vec<f32> = map_grid(60).map(|(x, z)| same.height_at(x, z)).collect();
        let other_values: Vec<f32> = map_grid(60).map(|(x, z)| other.height_at(x, z)).collect();

        assert_eq!(values, repeated);
        assert_ne!(values, other_values);
        assert!(values.iter().all(|value| (-1.0..=1.0).contains(value)));

        let mean = values.iter().sum::<f32>() / values.len() as f32;
        let deviation =
            (values.iter().map(|v| (v - mean).powi(2)).sum::<f32>() / values.len() as f32).sqrt();
        assert!(
            deviation > 0.1,
            "manchas quase constantes: desvio {deviation}"
        );
    }

    /// Testa que a clareira de spawn continua branca com a cor nova.
    #[test]
    fn europa_terrain_color_keeps_the_spawn_clearing_white() {
        let color = europa_terrain_color(&dtm(), 204, ICE, VALLEY);
        for (x, z) in [(0.0, 0.0), (100.0, 0.0), (0.0, -140.0), (-90.0, 90.0)] {
            assert_eq!(color.color_at(x, z), ICE, "({x}, {z}) deveria ser branco");
        }
    }

    /// Testa, no DTM real, que os pontos bem abaixo do entorno ficam em média mais tingidos que
    /// os pontos bem acima dele, e que nenhum ponto fica mais escuro que `terrain_valley_color`.
    #[test]
    fn europa_terrain_color_tints_low_ground_more_than_high_ground() {
        let dtm = dtm();
        let surroundings = dtm.blurred(RELATIVE_HEIGHT_BLUR_SIGMA_M);
        let color = europa_terrain_color(&dtm, 204, ICE, VALLEY);

        let (mut low, mut high) = (Vec::new(), Vec::new());
        for (x, z) in map_grid(80) {
            let red = color.color_at(x, z)[0];
            assert!(
                red >= VALLEY[0] - 1e-5,
                "mais escuro que a cor de vale em ({x}, {z})"
            );
            let relative = dtm.height_at(x, z) - surroundings.height_at(x, z);
            if relative < -RELATIVE_HEIGHT_TINT_FULL_M {
                low.push(red);
            } else if relative > RELATIVE_HEIGHT_TINT_FULL_M {
                high.push(red);
            }
        }

        let mean = |values: &[f32]| values.iter().sum::<f32>() / values.len() as f32;
        assert!(!low.is_empty() && !high.is_empty());
        assert!(
            mean(&low) < mean(&high) - 0.05,
            "vales {} deveriam ser mais escuros que cristas {}",
            mean(&low),
            mean(&high)
        );
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
