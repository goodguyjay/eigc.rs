use crate::chunk_mesh::{ChunkRegion, build_chunk_mesh};
use crate::height::{ColorSource, HeightSource};
use crate::params::TerrainParams;
use bevy::prelude::{Mesh, Vec2};

/// Gera uma malha de terreno a partir de parâmetros e uma fonte de altura, sem cor por vértice.
pub fn build_terrain_mesh(p: TerrainParams, height: &dyn HeightSource) -> Mesh {
    build_terrain_mesh_with_color(p, height, None)
}

/// Gera uma malha de terreno a partir de parâmetros e uma fonte de altura. Quando `color` é
/// `Some`, escreve também `Mesh::ATTRIBUTE_COLOR` por vértice a partir dela.
///
/// É a malha monolítica: um único chunk cobrindo o terreno inteiro, sem skirt. O pipeline de
/// LOD usa [`build_chunk_mesh`] diretamente.
pub fn build_terrain_mesh_with_color(
    p: TerrainParams,
    height: &dyn HeightSource,
    color: Option<&dyn ColorSource>,
) -> Mesh {
    let region = ChunkRegion {
        center: Vec2::ZERO,
        size: p.size,
    };
    build_chunk_mesh(region, p.res, 0.0, p.size, height, color)
}
