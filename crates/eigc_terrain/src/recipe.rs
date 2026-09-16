//! Monta a recipe de geração de terreno para cada lua suportada.
//! Esse módulo é o único lugar onde MoonId decide qual composição de HeightSource e quais
//! parâmetros visuais correspondem a uma lua específica.

use crate::height::comb::{Add2, Bias, FlattenColorNearOrigin, FlattenNearOrigin, Scale};
use crate::height::linea::{LineaColorField, LineaField, generate_linea_specs};
use crate::height::noise::PerlinFbm;
use crate::height::warp::Warp2D;
use crate::height::{ColorFn, HeightFn, arc, arc_color};
use crate::params::TerrainParams;
use crate::pipeline::TerrainAppearance;
use crate::systems::TerrainMaterialProperties;
use bevy::prelude::{Color, Vec2};
use eigc_moons::profile::{MoonId, MoonProfile};
use noise::Perlin;

/// Quantidade de lineae geradas no recorte (2 protagonistas + regulares) no momento.
const LINEA_COUNT: usize = 8;

/// Amplitude do ruído de detalhe fino na encosta das lineae, em metros.
const LINEA_DETAIL_AMPLITUDE_M: f32 = 3.0;

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
    // Tamanho do recorte, em metros. V1 do terreno: fundação pra iteração futura (LOD incluso),
    // aumentado de 6000 pra acompanhar o novo comprimento de linea (6000-14000m, ver
    // LINEA_LENGTH_MIN_M/MAX_M em height/linea.rs).
    const TERRAIN_SIZE_M: f32 = 22000.0;
    // Resolução da malha (vértices por lado). 
    // 
    // TODO (jay): Deliberadamente NÃO alterada nesta mudança de escala.
    // Mapa maior com a mesma resolução degrada a legibilidade do ruído de detalhe fino
    // da encosta (LINEA_DETAIL_AMPLITUDE_M), aceito por ora até LOD existir (ver BACKLOG.md).
    // Ainda uma malha única sem LOD.
    const TERRAIN_RES: u32 = 1536;

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

    // Ruído "ridged" (cristas isotrópicas fora do eixo das lineae) DESLIGADO por ora:
    // mesmo rebaixado a amplitude baixa (scale: 0.15), ainda produzia bumps isolados lidos como
    // "crateras" em lugares estranhos do terreno; artefato do PerlinRidged fora de contexto. Bloco 
    // mantido comentado (não removido) pra religar fácil depois
    // let ridged_noise = PerlinRidged {
    //     perlin: Perlin::new(seed ^ 0xB529_7A4D),
    //     freq: base_frequency * 2.5,
    //     octaves: 4,
    //     lacunarity: 2.2,
    //     gain: 0.75,
    //     amplitude: 1.0,
    //     z_anisotropy: 2.0,
    // };
    //
    // let oriented_ridges = Oriented {
    //     source: ridged_noise,
    //     dir: feature_direction,
    //     main_scale: 1.0,
    //     ortho_scale: 0.35,
    // };
    //
    // let subtle_ridges = Scale {
    //     s: oriented_ridges,
    //     scale: 0.15,
    // };

    let combined_features = base_noise;

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
        scale: calibration.vertical_amplitude_meters,
    };

    // exclui células da grade de posicionamento dentro do alcance total da clareira de spawn
    // (platô + blend), pra nenhuma linea nascer ancorada em cima do platô.
    let spawn_exclusion_radius_m = SPAWN_CLEARING_RADIUS_M + SPAWN_CLEARING_BLEND_M;
    let linea_specs = generate_linea_specs(
        seed,
        feature_direction,
        TERRAIN_SIZE_M * 0.5,
        LINEA_COUNT,
        spawn_exclusion_radius_m,
    );

    let linea_height = LineaField {
        specs: linea_specs.clone(),
        detail_noise: Perlin::new(seed ^ 0x1B87_3593),
        detail_amplitude_m: LINEA_DETAIL_AMPLITUDE_M,
        detail_frequency: base_frequency * 40.0,
    };

    // lineae somadas em metros absolutos depois do Scale: a escala física das cristas (100-300m)
    // não deve depender do knob vertical_amplitude_meters do ruído de fundo.
    let terrain_with_linea = Add2 {
        a: scaled_terrain,
        b: linea_height,
    };

    // bias: -0.1 é um ajuste estético. Não corresponde à precisão dos dados reais de Europa.
    // TODO (leticia.rodrigues): testar visualmente quando a cena tiver câmera/luz.
    let biased_terrain = Bias {
        s: terrain_with_linea,
        bias: -0.1,
    };

    // clareira plana na origem: garante um ponto de spawn legível e sem relevo caótico, com
    // transição suave até o terreno completo (ver SPAWN_CLEARING_RADIUS_M/BLEND_M). A altura do
    // platô é derivada do relevo real na borda do blend (média ao redor do perímetro).
    let terrain_with_clearing =
        FlattenNearOrigin::new(biased_terrain, SPAWN_CLEARING_RADIUS_M, SPAWN_CLEARING_BLEND_M);

    let linea_color = LineaColorField {
        specs: linea_specs,
        ridge_color: profile.terrain_base_color,
        valley_color: profile.terrain_valley_color,
    };

    // cor da clareira acompanha o mesmo raio/blend da altura: gelo claro e uniforme, não a cor
    // de vale que uma linea próxima da origem poderia produzir.
    let color_with_clearing = FlattenColorNearOrigin {
        source: linea_color,
        flat_color: profile.terrain_base_color,
        flat_radius_m: SPAWN_CLEARING_RADIUS_M,
        blend_radius_m: SPAWN_CLEARING_BLEND_M,
    };

    let terrain_params = TerrainParams {
        size: TERRAIN_SIZE_M,
        res: TERRAIN_RES,
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
        height: arc(terrain_with_clearing),
        params: terrain_params,
        appearance: terrain_appearance,
        material_properties,
        color: Some(arc_color(color_with_clearing)),
    }
}

/// Converte um array [r, g, b, a] em componentes lineares (formato salvo no .ron) para o tipo Color do bevy
fn color_from_linear_rgba(components: [f32; 4]) -> Color {
    Color::srgba(components[0], components[1], components[2], components[3])
}
