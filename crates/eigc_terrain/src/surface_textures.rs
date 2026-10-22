//! Texturas PBR tileáveis da superfície do terreno, por lua.

use bevy::image::{ImageAddressMode, ImageLoaderSettings, ImageSampler, ImageSamplerDescriptor};
use bevy::prelude::{AssetServer, Handle, Image, Resource};
use eigc_moons::profile::MoonId;

/// Caminho (relativo à raiz de assets) do diffuse/albedo de `snow_02`.
const SNOW_02_DIFFUSE_PATH: &str = "terrain/materials/snow_02/snow_02_diff_2k.jpg";
/// Caminho do normal map de `snow_02` (convenção OpenGL, a que o Bevy espera).
const SNOW_02_NORMAL_PATH: &str = "terrain/materials/snow_02/snow_02_nor_gl_2k.jpg";
/// Caminho do mapa ARM de `snow_02` (AO no canal R, roughness no G, metallic no B).
const SNOW_02_ARM_PATH: &str = "terrain/materials/snow_02/snow_02_arm_2k.jpg";

/// Anisotropia máxima do sampler.
const ANISOTROPY_CLAMP: u16 = 16;

/// Handles das texturas PBR da superfície do terreno.
#[derive(Resource, Clone, Debug)]
pub struct TerrainSurfaceTextures {
    /// Albedo, em sRGB.
    pub diffuse: Handle<Image>,
    /// Normal map, em espaço linear.
    pub normal: Handle<Image>,
    /// Mapa ARM empacotado (AO, roughness, metallic), em espaço linear.
    pub arm: Handle<Image>,
}

/// Texturas de superfície da lua, ou `None` quando a lua ainda não tem material próprio (o
/// terreno cai na cor/roughness do perfil).
pub fn surface_textures_for(
    moon_id: MoonId,
    asset_server: &AssetServer,
) -> Option<TerrainSurfaceTextures> {
    match moon_id {
        MoonId::Europa => Some(load_snow_02(asset_server)),
        MoonId::Io | MoonId::Ganymede | MoonId::Callisto => None,
    }
}

/// Carrega o material `snow_02` (Poly Haven, CC0, tile físico de 2,0 m).
fn load_snow_02(asset_server: &AssetServer) -> TerrainSurfaceTextures {
    TerrainSurfaceTextures {
        diffuse: load_repeating(asset_server, SNOW_02_DIFFUSE_PATH, true),
        normal: load_repeating(asset_server, SNOW_02_NORMAL_PATH, false),
        arm: load_repeating(asset_server, SNOW_02_ARM_PATH, false),
    }
}

fn load_repeating(asset_server: &AssetServer, path: &'static str, is_srgb: bool) -> Handle<Image> {
    asset_server.load_with_settings(path, move |settings: &mut ImageLoaderSettings| {
        apply_tiling_settings(settings, is_srgb);
    })
}

/// Configura o loader para uma textura que repete: sampler linear com `Repeat` nos três eixos e
/// filtro anisotrópico.
/// `is_srgb` precisa ser `false` para normal map e ARM, que guardam dados e não cor.
fn apply_tiling_settings(settings: &mut ImageLoaderSettings, is_srgb: bool) {
    settings.is_srgb = is_srgb;
    settings.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor {
        address_mode_u: ImageAddressMode::Repeat,
        address_mode_v: ImageAddressMode::Repeat,
        address_mode_w: ImageAddressMode::Repeat,
        anisotropy_clamp: ANISOTROPY_CLAMP,
        ..ImageSamplerDescriptor::linear()
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Testa se o loader de texturas aplica corretamente o sampler com `Repeat` nos três eixos e
    /// respeita o espaço de cor (sRGB ou linear).
    #[test]
    fn tiling_settings_repeat_on_every_axis_and_respect_color_space() {
        let mut settings = ImageLoaderSettings::default();

        apply_tiling_settings(&mut settings, false);

        assert!(!settings.is_srgb);
        let ImageSampler::Descriptor(descriptor) = &settings.sampler else {
            panic!("sampler deveria ser um descriptor explícito");
        };
        assert_eq!(descriptor.address_mode_u, ImageAddressMode::Repeat);
        assert_eq!(descriptor.address_mode_v, ImageAddressMode::Repeat);
        assert_eq!(descriptor.address_mode_w, ImageAddressMode::Repeat);
        assert_eq!(descriptor.anisotropy_clamp, ANISOTROPY_CLAMP);
    }
}
