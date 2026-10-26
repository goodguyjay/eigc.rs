//! Monta a recipe de geração de terreno para cada lua suportada.
//! Esse módulo é o único lugar onde MoonId decide qual composição de HeightSource e quais
//! parâmetros visuais correspondem a uma lua específica.

use crate::height::comb::{Add2, Bias, FlattenColorNearOrigin, FlattenNearOrigin, Scale};
use crate::height::heightmap::HeightmapHeight;
use crate::height::noise::PerlinFbm;
use crate::height::slope::SlopeColorField;
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

/// Amplitude da rugosidade fina procedural somada por cima do DTM, em metros. É a única parte
/// sintética do relevo: nenhuma fonte real cobre a escala de passo humano. A calibrar.
const DETAIL_AMPLITUDE_M: f32 = 1.5;

/// Quantas vezes a frequência base do perfil é multiplicada para a rugosidade fina (comprimento
/// de onda de ~60 m com `base_frequency` de 1/600 m), de modo que o ruído fique acima do
/// espaçamento de ~10,7 m do nível 0 de LOD.
const DETAIL_FREQUENCY_FACTOR: f32 = 10.0;

/// Distância, em metros, entre as amostras usadas para estimar a inclinação do DTM na cor.
/// Da ordem de meio pixel do DTM (227,65 m).
const SLOPE_SAMPLE_STEP_M: f32 = 100.0;

/// Inclinação do DTM (subida/avanço) abaixo da qual o terreno recebe só a cor de gelo plano.
/// A inclinação média do recorte é ~0,05 e o percentil 95 ~0,16.
const SLOPE_COLOR_START: f32 = 0.06;

/// Inclinação do DTM (subida/avanço) a partir da qual o terreno recebe totalmente a cor de vale.
const SLOPE_COLOR_END: f32 = 0.20;

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
    /// Fonte de cor por vértice, quando a receita pinta feições distintas (ex.: lineae). `None`
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
/// verticalmente, por cima entra só uma rugosidade fina procedural. Cor por inclinação do DTM.
fn europa_recipe(profile: &MoonProfile) -> TerrainRecipe {
    // Tamanho do recorte, em metros. Casa com o DTM (~22082 m); o excedente de ~41 m por lado
    // do DTM fica fora do terreno.
    const TERRAIN_SIZE_M: f32 = 22000.0;
    // Resolução da malha monolítica legada (vértices por lado). O terreno em jogo usa chunks
    // com LOD (ver `lod` abaixo), então isto só alimenta `TerrainParams.res` (ver BACKLOG.md).
    const TERRAIN_RES: u32 = 1536;

    let calibration = &profile.terrain;

    let seed = calibration.seed;
    let base_frequency = calibration.base_frequency;

    let feature_direction = Vec2::from(calibration.feature_direction).normalize();

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
        line_dir: feature_direction,
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

    let fine_detail = PerlinFbm {
        perlin: Perlin::new(seed),
        freq: base_frequency * DETAIL_FREQUENCY_FACTOR,
        octaves: 3,
        lacunarity: 2.0,
        gain: 0.5,
        amplitude: DETAIL_AMPLITUDE_M,
    };

    let hybrid_terrain = Add2 {
        a: real_relief,
        b: fine_detail,
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
        steep_color: profile.terrain_valley_color,
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

    let terrain_appearance = TerrainAppearance {
        base_color: color_from_linear_rgba(profile.terrain_base_color),
        display_name: profile.display_name.clone(),
    };

    let material_properties = TerrainMaterialProperties {
        perceptual_roughness: calibration.perceptual_roughness,
        reflectance: calibration.reflectance,
    };

    // 64x64 chunks de 343,75 m. O nível 0 tem espaçamento de ~10,7 m (melhor que os 14,3 m da
    // malha monolítica) e o mais grosso ~86 m, ainda menor que a largura das lineae (100-300 m).
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
        color: Some(arc_color(color_with_clearing)),
        lod,
    }
}

/// Converte um array [r, g, b, a] em componentes lineares (formato salvo no .ron) para o tipo Color do bevy
fn color_from_linear_rgba(components: [f32; 4]) -> Color {
    Color::srgba(components[0], components[1], components[2], components[3])
}
