//! Este módulo contém sistemas para gerar e renderizar o terreno no Bevy.

use crate::height::HeightSource;
use crate::pipeline::{HeightResource, TerrainAppearance};
use crate::{mesh::build_terrain_mesh, params::TerrainParams};
use bevy::prelude::{Assets, Commands, Mesh, Mesh3d, MeshMaterial3d, Name, Res, ResMut, StandardMaterial, Transform};

/// Spawna uma entidade de terreno no Bevy usando os parâmetros e a função de altura fornecidos.
pub fn build_and_spawn_terrain(
    commands:&mut Commands,
    meshes: &mut ResMut<Assets<Mesh>>,
    materials: &mut Assets<StandardMaterial>,
    params: TerrainParams,
    height: &dyn HeightSource,
    appearance: &TerrainAppearance,
) {
    let mesh = build_terrain_mesh(params, height);
    let mesh_handle = meshes.add(mesh);

    let material = materials.add(StandardMaterial {
        base_color: appearance.base_color,
        perceptual_roughness: 0.7,
        reflectance: 0.5,
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


pub fn spawn_terrain(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    terrain_params: Res<TerrainParams>,
    height_resource: Res<HeightResource>,
    terrain_appearance: Res<TerrainAppearance>
) {
    build_and_spawn_terrain(
        &mut commands,
        &mut meshes,
        &mut materials,
        *terrain_params,
        height_resource.0.as_ref(),
        &terrain_appearance
    );
}