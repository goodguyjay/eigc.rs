//! Monta a recipe de geração de terreno para cada lua suportada.
//! Esse módulo é o único lugar onde MoonId decide qual composição de HeightSource e quais
//! parâmetros visuais correspondem a uma lua específica. Cada lua calibrada tem um submódulo
//! próprio; aqui ficam só o ponto de entrada e o que as receitas compartilham.

mod europa;
mod io;

use crate::height::{ColorFn, HeightFn};
use crate::lod::TerrainLodConfig;
use crate::params::TerrainParams;
use crate::pipeline::TerrainAppearance;
use crate::systems::TerrainMaterialProperties;
use bevy::prelude::Color;
use eigc_moons::profile::{MoonId, MoonProfile};

/// Exagero vertical aplicado ao relevo (ver `TerrainParams::vertical_exaggeration`). `1.0`
/// mantém a elevação medida no DTM de Europa.
const VERTICAL_EXAGGERATION: f32 = 1.0;

/// Tamanho do recorte, em metros, igual em todas as luas para câmera, LOD e spawn de debug
/// valerem igual. Casa com o DTM de Europa (~22082 m); o excedente de ~41 m por lado do DTM fica
/// fora do terreno.
const TERRAIN_SIZE_M: f32 = 22000.0;

/// Resolução da malha monolítica legada (vértices por lado). O terreno em jogo usa chunks com LOD
/// (ver `chunked_lod_config`), então isto só alimenta `TerrainParams.res` (ver BACKLOG.md).
const TERRAIN_RES: u32 = 1536;

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
    /// Fonte de cor por vértice, quando a receita pinta feições distintas (ex.: cor por
    /// inclinação). `None` significa que o terreno usa só a cor flat de `appearance.base_color`.
    pub color: Option<ColorFn>,
    /// Configuração do LOD por chunks do terreno.
    pub lod: TerrainLodConfig,
}

/// Ponto de entrada para montar a receita de geração de terreno de uma lua específica.
pub fn build_recipe(profile: &MoonProfile) -> TerrainRecipe {
    match profile.moon_id {
        MoonId::Europa => europa::europa_recipe(profile),
        MoonId::Io => io::io_recipe(profile),
        MoonId::Ganymede => unimplemented!("receita de Ganymede ainda não calibrada"),
        MoonId::Callisto => unimplemented!("receita de Callisto ainda não calibrada"),
    }
}

/// Configuração de LOD por chunks compartilhada pelas receitas de recorte de ~22 km.
fn chunked_lod_config() -> TerrainLodConfig {
    // 64x64 chunks de 343,75 m. O nível 0 tem espaçamento de ~5,4 m (melhor que os 14,3 m da
    // malha monolítica) e o mais grosso ~43 m, menor que a largura das cristas das lineae de
    // Europa (100 a 400 m).
    // O nível de cada chunk sai do erro geométrico medido (ver `lod_error.rs`) projetado na tela:
    // `max_screen_error_px` menor = mais detalhe de mais longe. `min_error_fraction` é o piso
    // para chunks lisos (e para a cor por vértice, que também perde resolução ao engrossar).
    // Valores calibráveis: medir com o app rodando antes de mexer.
    TerrainLodConfig {
        chunks_per_side: 64,
        quads_per_chunk: [64, 32, 16, 8],
        max_screen_error_px: 1.5,
        min_error_fraction: 0.2,
        hysteresis_fraction: 0.1,
        skirt_depth_factor: 0.35,
        in_flight_vertex_budget: 30_000,
        max_error_tasks_in_flight: 16,
    }
}

/// Aparência e material comuns a toda receita, lidos direto do perfil.
fn appearance_and_material(
    profile: &MoonProfile,
) -> (TerrainAppearance, TerrainMaterialProperties) {
    (
        TerrainAppearance {
            base_color: color_from_linear_rgba(profile.terrain_base_color),
            display_name: profile.display_name.clone(),
        },
        TerrainMaterialProperties {
            perceptual_roughness: profile.terrain.perceptual_roughness,
            reflectance: profile.terrain.reflectance,
        },
    )
}

/// Converte um array [r, g, b, a] em componentes lineares (formato salvo no .ron) para o tipo Color do bevy
fn color_from_linear_rgba(components: [f32; 4]) -> Color {
    Color::srgba(components[0], components[1], components[2], components[3])
}
