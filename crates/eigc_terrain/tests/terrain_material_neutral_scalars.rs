use bevy::prelude::{Color, Handle, Image};
use eigc_terrain::pipeline::TerrainAppearance;
use eigc_terrain::surface_textures::TerrainSurfaceTextures;
use eigc_terrain::systems::{TerrainMaterialProperties, build_terrain_material};

fn appearance() -> TerrainAppearance {
    TerrainAppearance {
        base_color: Color::srgb(0.2, 0.3, 0.4),
        display_name: "Teste".to_string(),
    }
}

fn properties() -> TerrainMaterialProperties {
    TerrainMaterialProperties {
        perceptual_roughness: 0.7,
        reflectance: 0.5,
    }
}

fn textures() -> TerrainSurfaceTextures {
    TerrainSurfaceTextures {
        diffuse: Handle::<Image>::default(),
        normal: Handle::<Image>::default(),
        arm: Handle::<Image>::default(),
    }
}

/// Com texturas, os escalares multiplicadores ficam neutros: branco, roughness 1.0 e metallic 0.0,
/// senão o `StandardMaterial` tinge ou apaga a textura.
#[test]
fn textured_material_keeps_every_scalar_multiplier_neutral() {
    let material = build_terrain_material(&appearance(), properties(), Some(&textures()));

    assert_eq!(material.base_color, Color::WHITE);
    assert_eq!(material.perceptual_roughness, 1.0);
    assert_eq!(material.metallic, 0.0);
}

/// O ARM empacotado alimenta o roughness/metallic (G/B) e o AO (R) pela mesma textura.
#[test]
fn textured_material_binds_all_maps_and_reads_arm_for_roughness_and_occlusion() {
    let material = build_terrain_material(&appearance(), properties(), Some(&textures()));

    assert!(material.base_color_texture.is_some());
    assert!(material.normal_map_texture.is_some());
    assert!(material.metallic_roughness_texture.is_some());
    assert_eq!(
        material.metallic_roughness_texture,
        material.occlusion_texture
    );
}

/// Sem texturas o material continua vindo da aparência e das propriedades do perfil.
#[test]
fn untextured_material_uses_appearance_and_profile_properties() {
    let material = build_terrain_material(&appearance(), properties(), None);

    assert_eq!(material.base_color, Color::srgb(0.2, 0.3, 0.4));
    assert_eq!(material.perceptual_roughness, 0.7);
    assert!(material.base_color_texture.is_none());
}
