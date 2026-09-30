//! Chunks do terreno: spawn inicial e troca de nível de LOD conforme a distância ao foco.
//!
//! Todos os chunks nascem de uma vez, com a malha do nível mais grosso, e daí em diante só
//! trocam de malha. Assim nunca falta terreno e o sistema de `Update` nunca faz `spawn`.

use crate::chunk_mesh::{ChunkRegion, build_chunk_mesh};
use crate::height::{ColorSource, HeightSource};
use crate::lod::{
    COARSEST_LEVEL, ChunkCoord, LOD_LEVEL_COUNT, TerrainLodConfig, chunk_center, chunk_size,
    chunk_vertex_count, plan_lod_updates,
};
use crate::params::TerrainParams;
use crate::pipeline::{ColorResource, HeightResource};
use bevy::prelude::{
    Assets, Commands, Component, Handle, Mesh, Mesh3d, MeshMaterial3d, Query, Res, ResMut,
    Resource, StandardMaterial, Transform,
};
use eigc_common::lod_focus::LodFocus;
use std::collections::HashMap;

/// Marca uma entidade de chunk do terreno e guarda sua posição na grade e o nível de LOD atual.
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub struct TerrainChunk {
    /// Posição do chunk na grade do terreno.
    pub coord: ChunkCoord,
    /// Nível de LOD da malha atual (0 é o mais fino).
    pub level: u8,
}

/// Estatísticas do LOD do terreno, atualizadas a cada frame por [`update_terrain_lod`].
#[derive(Resource, Default, Clone, Debug, PartialEq, Eq)]
pub struct TerrainLodStats {
    /// Quantidade de chunks em cada nível de LOD.
    pub chunks_per_level: [u32; LOD_LEVEL_COUNT],
    /// Total de vértices somados de todos os chunks (incluindo skirts).
    pub vertices_total: u64,
    /// Quantidade de malhas geradas no último frame.
    pub builds_last_frame: u32,
}

/// Spawna todos os chunks do terreno com a malha do nível mais grosso.
///
/// O `material` é compartilhado por todos os chunks.
pub fn spawn_terrain_chunks(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    material: Handle<StandardMaterial>,
    params: &TerrainParams,
    lod: &TerrainLodConfig,
    height: &dyn HeightSource,
    color: Option<&dyn ColorSource>,
) {
    for iz in 0..lod.chunks_per_side {
        for ix in 0..lod.chunks_per_side {
            let coord = ChunkCoord { ix, iz };
            let center = chunk_center(coord, params.size, lod.chunks_per_side);
            let mesh = build_level_mesh(coord, COARSEST_LEVEL, params.size, lod, height, color);

            commands.spawn((
                TerrainChunk {
                    coord,
                    level: COARSEST_LEVEL,
                },
                Mesh3d(meshes.add(mesh)),
                MeshMaterial3d(material.clone()),
                Transform::from_xyz(center.x, 0.0, center.y),
            ));
        }
    }
}

/// Sistema que troca a malha dos chunks conforme o nível de LOD pedido pela distância ao
/// [`LodFocus`], respeitando o orçamento de vértices por frame.
///
/// Não spawna nem despawna nada: só substitui o `Mesh3d` de chunks que já existem. O `Aabb` é
/// recalculado pelo Bevy quando o `Mesh3d` muda.
pub fn update_terrain_lod(
    mut meshes: ResMut<Assets<Mesh>>,
    focus: Res<LodFocus>,
    params: Res<TerrainParams>,
    lod: Res<TerrainLodConfig>,
    height: Res<HeightResource>,
    color: Option<Res<ColorResource>>,
    mut chunks: Query<(&mut TerrainChunk, &mut Mesh3d)>,
    mut stats: ResMut<TerrainLodStats>,
) {
    let plan = plan_lod_updates(
        chunks.iter().map(|(chunk, _)| (chunk.coord, chunk.level)),
        focus.position,
        &lod,
        params.size,
    );

    let mut builds_this_frame = 0;
    if !plan.is_empty() {
        let new_levels: HashMap<ChunkCoord, u8> = plan.into_iter().collect();
        let color_source = color.as_deref().map(|resource| resource.0.as_ref());

        for (mut chunk, mut mesh_handle) in &mut chunks {
            let Some(&new_level) = new_levels.get(&chunk.coord) else {
                continue;
            };
            let mesh = build_level_mesh(
                chunk.coord,
                new_level,
                params.size,
                &lod,
                height.0.as_ref(),
                color_source,
            );
            mesh_handle.0 = meshes.add(mesh);
            chunk.level = new_level;
            builds_this_frame += 1;
        }
    }

    let mut new_stats = TerrainLodStats {
        builds_last_frame: builds_this_frame,
        ..TerrainLodStats::default()
    };
    for (chunk, _) in &chunks {
        let level = usize::from(chunk.level);
        new_stats.chunks_per_level[level] += 1;
        new_stats.vertices_total += chunk_vertex_count(lod.quads_per_chunk[level]) as u64;
    }
    if *stats != new_stats {
        *stats = new_stats;
    }
}

/// Gera a malha de um chunk no nível pedido, com skirt proporcional ao espaçamento do nível.
fn build_level_mesh(
    coord: ChunkCoord,
    level: u8,
    terrain_size: f32,
    lod: &TerrainLodConfig,
    height: &dyn HeightSource,
    color: Option<&dyn ColorSource>,
) -> Mesh {
    let size = chunk_size(terrain_size, lod.chunks_per_side);
    let quads = lod.quads_per_chunk[usize::from(level)];
    let skirt_depth = lod.skirt_depth_factor * size / quads as f32;
    let region = ChunkRegion {
        center: chunk_center(coord, terrain_size, lod.chunks_per_side),
        size,
    };
    build_chunk_mesh(region, quads, skirt_depth, terrain_size, height, color)
}
