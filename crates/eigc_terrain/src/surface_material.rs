//! Material do terreno com mistura de duas escalas de textura.
//!
//! A textura de superfície tem ~1 mm por texel na escala física (tile de 2 m).
use crate::surface_textures::TerrainSurfaceTextures;
use bevy::asset::Asset;
use bevy::math::Vec4;
use bevy::pbr::{ExtendedMaterial, MaterialExtension, StandardMaterial};
use bevy::prelude::{Handle, Image};
use bevy::reflect::TypePath;
use bevy::render::render_resource::AsBindGroup;
use bevy::shader::ShaderRef;

/// `StandardMaterial` estendido com a camada de textura em escala longa.
pub type TerrainMaterial = ExtendedMaterial<StandardMaterial, TerrainDetailExtension>;

/// Caminho (relativo à raiz de assets) do shader de mistura.
const DETAIL_SHADER_PATH: &str = "shaders/terrain_detail.wgsl";

/// Quantas vezes a escala longa é maior que a curta (tile de 2 m x 8 = 16 m).
const MACRO_SCALE_RATIO: f32 = 8.0;

/// Distância à câmera, em metros, onde a escala longa começa a entrar.
const BLEND_START_M: f32 = 4.0;

/// Distância à câmera, em metros, a partir da qual só a escala longa vale.
const BLEND_END_M: f32 = 30.0;

/// Camada de textura em escala longa do terreno: as mesmas imagens do material base, amostradas
/// com UV dividida por [`MACRO_SCALE_RATIO`]. Sem texturas, a camada fica desligada.
#[derive(Asset, AsBindGroup, TypePath, Debug, Clone)]
pub struct TerrainDetailExtension {
    /// `x`: razão entre a escala longa e a curta, `y`: início da mistura (m), `z`: fim da mistura
    /// (m), `w`: 1.0 com a camada habilitada.
    #[uniform(100)]
    pub params: Vec4,
    /// Albedo em escala longa.
    #[texture(101)]
    #[sampler(102)]
    #[dependency]
    pub macro_diffuse: Option<Handle<Image>>,
    /// Normal map em escala longa.
    #[texture(103)]
    #[sampler(104)]
    #[dependency]
    pub macro_normal: Option<Handle<Image>>,
    /// Mapa ARM (AO, roughness, metallic) em escala longa.
    #[texture(105)]
    #[sampler(106)]
    #[dependency]
    pub macro_arm: Option<Handle<Image>>,
}

impl MaterialExtension for TerrainDetailExtension {
    fn fragment_shader() -> ShaderRef {
        ShaderRef::Path(DETAIL_SHADER_PATH.into())
    }
}

/// Monta a extensão a partir das texturas de superfície. `None` desliga a camada em escala longa
/// (o material cai só no que o `StandardMaterial` base já faz).
pub fn build_terrain_extension(surface: Option<&TerrainSurfaceTextures>) -> TerrainDetailExtension {
    let Some(surface) = surface else {
        return TerrainDetailExtension {
            params: Vec4::new(MACRO_SCALE_RATIO, BLEND_START_M, BLEND_END_M, 0.0),
            macro_diffuse: None,
            macro_normal: None,
            macro_arm: None,
        };
    };

    TerrainDetailExtension {
        params: Vec4::new(MACRO_SCALE_RATIO, BLEND_START_M, BLEND_END_M, 1.0),
        macro_diffuse: Some(surface.diffuse.clone()),
        macro_normal: Some(surface.normal.clone()),
        macro_arm: Some(surface.arm.clone()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn textures() -> TerrainSurfaceTextures {
        TerrainSurfaceTextures {
            diffuse: Handle::<Image>::default(),
            normal: Handle::<Image>::default(),
            arm: Handle::<Image>::default(),
        }
    }

    /// Testa se a extensão com texturas habilita a camada macro e vincula todos os mapas.
    #[test]
    fn extension_with_textures_enables_the_macro_layer_and_binds_every_map() {
        let extension = build_terrain_extension(Some(&textures()));

        assert_eq!(extension.params.w, 1.0);
        assert!(extension.macro_diffuse.is_some());
        assert!(extension.macro_normal.is_some());
        assert!(extension.macro_arm.is_some());
    }

    /// Testa se a extensão sem texturas desabilita a camada macro e não vincula nenhum mapa.
    #[test]
    fn extension_without_textures_disables_the_macro_layer() {
        let extension = build_terrain_extension(None);

        assert_eq!(extension.params.w, 0.0);
        assert!(extension.macro_diffuse.is_none());
    }

    /// Testa se a extensão com texturas tem parâmetros coerentes: a escala macro é maior que 1.0 e
    /// a distância de início da mistura é menor que a distância de fim da mistura.
    #[test]
    fn blend_starts_before_it_ends_and_macro_scale_is_larger_than_micro() {
        let extension = build_terrain_extension(Some(&textures()));

        assert!(extension.params.x > 1.0);
        assert!(extension.params.y < extension.params.z);
    }
}
