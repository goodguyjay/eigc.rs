//! Este módulo contém sistemas para gerar e renderizar o terreno no Bevy.

use crate::height::{ColorSource, HeightSource};
use crate::pipeline::{ColorResource, HeightResource, TerrainAppearance};
use crate::{mesh::build_terrain_mesh_with_color, params::TerrainParams};
use bevy::prelude::{Assets, Commands, Mesh, Mesh3d, MeshMaterial3d, Name, Res, ResMut, Resource, StandardMaterial, Transform};

/// Propriedades físicas do material de terreno derivadas de TerrainCalibration
#[derive(Resource, Copy, Clone)]
pub struct TerrainMaterialProperties {
    pub perceptual_roughness: f32,
    pub reflectance: f32,
}

/// Spawna uma entidade de terreno no Bevy usando os parâmetros e a função de altura fornecidos,
/// sem cor por vértice.
pub fn build_and_spawn_terrain(
    commands: &mut Commands,
    meshes: &mut ResMut<Assets<Mesh>>,
    materials: &mut Assets<StandardMaterial>,
    params: TerrainParams,
    height: &dyn HeightSource,
    appearance: &TerrainAppearance,
    material_properties: TerrainMaterialProperties,
) {
    build_and_spawn_terrain_with_color(
        commands,
        meshes,
        materials,
        params,
        height,
        None,
        appearance,
        material_properties,
    );
}

/// Spawna uma entidade de terreno no Bevy usando os parâmetros, a função de altura e,
/// opcionalmente, uma fonte de cor por vértice fornecidos.
pub fn build_and_spawn_terrain_with_color(
    commands: &mut Commands,
    meshes: &mut ResMut<Assets<Mesh>>,
    materials: &mut Assets<StandardMaterial>,
    params: TerrainParams,
    height: &dyn HeightSource,
    color: Option<&dyn ColorSource>,
    appearance: &TerrainAppearance,
    material_properties: TerrainMaterialProperties,
) {
    let mesh = build_terrain_mesh_with_color(params, height, color);
    let mesh_handle = meshes.add(mesh);

    let material = materials.add(StandardMaterial {
        base_color: appearance.base_color,
        perceptual_roughness: material_properties.perceptual_roughness,
        reflectance: material_properties.reflectance,
        metallic: 0.0,
        ..Default::default()
    });

    commands.spawn((
        Mesh3d(mesh_handle),
        MeshMaterial3d(material),
        Transform::default(),
        Name::new(appearance.display_name.clone()),
    ));
}

/// Sistema Bevy que lê os recursos de terreno já inseridos e spawna a entidade de terreno. A cor
/// por vértice é opcional: quando `ColorResource` não foi inserido pela receita, o terreno usa só
/// a cor flat do material.
pub fn spawn_terrain(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    terrain_params: Res<TerrainParams>,
    height_resource: Res<HeightResource>,
    color_resource: Option<Res<ColorResource>>,
    terrain_appearance: Res<TerrainAppearance>,
    material_properties: Res<TerrainMaterialProperties>,
) {
    build_and_spawn_terrain_with_color(
        &mut commands,
        &mut meshes,
        &mut materials,
        *terrain_params,
        height_resource.0.as_ref(),
        color_resource.as_deref().map(|c| c.0.as_ref()),
        &terrain_appearance,
        *material_properties,
    );
}
