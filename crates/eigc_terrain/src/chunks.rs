//! Chunks do terreno: spawn inicial, medição de erro geométrico e troca de nível de LOD.
//!
//! Todos os chunks nascem de uma vez, com a malha do nível mais grosso, e daí em diante só
//! trocam de malha. Assim nunca falta terreno e nenhum sistema de `Update` faz `spawn`.
//!
//! O trabalho pesado roda fora da thread principal (`AsyncComputeTaskPool`):
//! - a medição do erro de cada nível de cada chunk ([`measure_chunk_errors`]);
//! - a construção das malhas novas ([`update_terrain_lod`] começa, [`finish_chunk_builds`]
//!   termina). Uma construção obsoleta é cancelada quando o chunk volta a querer o nível atual.

use crate::chunk_mesh::{ChunkRegion, build_chunk_mesh};
use crate::height::{ColorFn, ColorSource, HeightFn, HeightSource};
use crate::lod::{
    COARSEST_LEVEL, ChunkCoord, ChunkLodInput, LOD_LEVEL_COUNT, TerrainLodConfig, chunk_center,
    chunk_size, chunk_vertex_count, distance_to_chunk, level_distance_thresholds,
    plan_lod_updates,
};
use crate::lod_error::{LevelErrors, measure_level_errors};
use crate::params::TerrainParams;
use crate::pipeline::{ColorResource, HeightResource};
use bevy::prelude::{
    Assets, Commands, Component, Entity, Handle, Mesh, Mesh3d, MeshMaterial3d, Query, Res, ResMut,
    Resource, StandardMaterial, Transform,
};
use bevy::tasks::{AsyncComputeTaskPool, Task, block_on, poll_once};
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

/// Malha de um chunk sendo construída em segundo plano. Descartar o componente cancela a tarefa
/// se ela ainda não começou a rodar.
#[derive(Component)]
pub struct PendingChunkMesh {
    /// Nível de LOD da malha em construção.
    pub(crate) target_level: u8,
    pub(crate) task: Task<Mesh>,
}

/// Estatísticas do LOD do terreno, atualizadas a cada frame por [`update_terrain_lod`].
#[derive(Resource, Default, Clone, Debug, PartialEq, Eq)]
pub struct TerrainLodStats {
    /// Quantidade de chunks em cada nível de LOD.
    pub chunks_per_level: [u32; LOD_LEVEL_COUNT],
    /// Total de vértices somados de todos os chunks (incluindo skirts).
    pub vertices_total: u64,
    /// Quantidade de malhas que começaram a ser construídas no último frame.
    pub builds_last_frame: u32,
    /// Quantidade de malhas em construção em segundo plano.
    pub pending_builds: u32,
    /// Quantidade de chunks cujo erro geométrico já foi medido.
    pub measured_chunks: u32,
}

/// Estado da medição de erro de um chunk.
enum ErrorEntry {
    Unmeasured,
    InFlight(Task<LevelErrors>),
    Done(LevelErrors),
}

/// Erro geométrico medido de cada nível de cada chunk, preenchido em segundo plano.
///
/// Enquanto um chunk não foi medido ele fica no nível mais grosso.
#[derive(Resource, Default)]
pub struct ChunkErrorTable {
    entries: Vec<ErrorEntry>,
}

impl ChunkErrorTable {
    fn index(coord: ChunkCoord, chunks_per_side: u32) -> usize {
        (coord.iz * chunks_per_side + coord.ix) as usize
    }

    fn measured(&self, coord: ChunkCoord, chunks_per_side: u32) -> Option<&LevelErrors> {
        match self.entries.get(Self::index(coord, chunks_per_side)) {
            Some(ErrorEntry::Done(errors)) => Some(errors),
            _ => None,
        }
    }

    /// Quantidade de chunks com erro já medido.
    pub fn measured_count(&self) -> usize {
        self.entries
            .iter()
            .filter(|entry| matches!(entry, ErrorEntry::Done(_)))
            .count()
    }
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

/// Sistema que mede, em segundo plano, o erro geométrico de cada nível de cada chunk.
///
/// Recolhe as medições prontas e inicia novas, as mais próximas do foco primeiro, até
/// `max_error_tasks_in_flight` ao mesmo tempo.
pub fn measure_chunk_errors(
    focus: Res<LodFocus>,
    params: Res<TerrainParams>,
    lod: Res<TerrainLodConfig>,
    height: Res<HeightResource>,
    mut table: ResMut<ChunkErrorTable>,
) {
    let chunks_per_side = lod.chunks_per_side;
    let chunk_count = (chunks_per_side * chunks_per_side) as usize;
    if table.entries.len() != chunk_count {
        table.entries = (0..chunk_count).map(|_| ErrorEntry::Unmeasured).collect();
    }

    let mut in_flight = 0_usize;
    for entry in &mut table.entries {
        if let ErrorEntry::InFlight(task) = entry {
            match block_on(poll_once(task)) {
                Some(errors) => *entry = ErrorEntry::Done(errors),
                None => in_flight += 1,
            }
        }
    }

    let slots = (lod.max_error_tasks_in_flight as usize).saturating_sub(in_flight);
    if slots == 0 {
        return;
    }

    let size = chunk_size(params.size, chunks_per_side);
    let mut unmeasured: Vec<(f32, usize)> = table
        .entries
        .iter()
        .enumerate()
        .filter(|(_, entry)| matches!(entry, ErrorEntry::Unmeasured))
        .map(|(index, _)| {
            let coord = ChunkCoord {
                ix: index as u32 % chunks_per_side,
                iz: index as u32 / chunks_per_side,
            };
            let center = chunk_center(coord, params.size, chunks_per_side);
            (distance_to_chunk(focus.position, center, size), index)
        })
        .collect();
    unmeasured.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));

    for &(_, index) in unmeasured.iter().take(slots) {
        let coord = ChunkCoord {
            ix: index as u32 % chunks_per_side,
            iz: index as u32 / chunks_per_side,
        };
        let center = chunk_center(coord, params.size, chunks_per_side);
        let height_source: HeightFn = height.0.clone();
        let config = lod.clone();
        let task = AsyncComputeTaskPool::get().spawn(async move {
            measure_level_errors(height_source.as_ref(), center, size, &config)
        });
        table.entries[index] = ErrorEntry::InFlight(task);
    }
}

/// Sistema que recolhe as malhas de chunk terminadas em segundo plano e as coloca nas entidades.
pub fn finish_chunk_builds(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut chunks: Query<(Entity, &mut TerrainChunk, &mut Mesh3d, &mut PendingChunkMesh)>,
) {
    for (entity, mut chunk, mut mesh_handle, mut pending) in &mut chunks {
        let Some(mesh) = block_on(poll_once(&mut pending.task)) else {
            continue;
        };
        // O Bevy recalcula o Aabb quando o Mesh3d muda.
        mesh_handle.0 = meshes.add(mesh);
        chunk.level = pending.target_level;
        commands.entity(entity).remove::<PendingChunkMesh>();
    }
}

/// Sistema que decide quais chunks trocam de nível e inicia a construção das malhas novas em
/// segundo plano, respeitando `in_flight_vertex_budget`.
///
/// O nível vem do erro geométrico medido do chunk, da distância ao [`LodFocus`] e da escala de
/// tela dele (ver [`level_distance_thresholds`]). Não spawna nem despawna nada.
pub fn update_terrain_lod(
    mut commands: Commands,
    focus: Res<LodFocus>,
    params: Res<TerrainParams>,
    lod: Res<TerrainLodConfig>,
    height: Res<HeightResource>,
    color: Option<Res<ColorResource>>,
    errors: Res<ChunkErrorTable>,
    chunks: Query<(Entity, &TerrainChunk, Option<&PendingChunkMesh>)>,
    mut stats: ResMut<TerrainLodStats>,
) {
    let size = chunk_size(params.size, lod.chunks_per_side);

    let inputs = chunks.iter().map(|(_, chunk, pending)| ChunkLodInput {
        coord: chunk.coord,
        level: chunk.level,
        pending: pending.map(|pending| pending.target_level),
        thresholds: level_distance_thresholds(
            errors.measured(chunk.coord, lod.chunks_per_side),
            size,
            &lod,
            focus.screen_scale,
        ),
    });
    let requests = plan_lod_updates(inputs, focus.position, &lod, params.size);

    let mut builds_started = 0;
    if !requests.is_empty() {
        let targets: HashMap<ChunkCoord, u8> = requests
            .into_iter()
            .map(|request| (request.coord, request.target))
            .collect();
        let pool = AsyncComputeTaskPool::get();

        for (entity, chunk, pending) in &chunks {
            let Some(&target) = targets.get(&chunk.coord) else {
                continue;
            };
            if target == chunk.level {
                if pending.is_some() {
                    commands.entity(entity).remove::<PendingChunkMesh>();
                }
                continue;
            }

            let height_source: HeightFn = height.0.clone();
            let color_source: Option<ColorFn> = color.as_ref().map(|resource| resource.0.clone());
            let config = lod.clone();
            let coord = chunk.coord;
            let terrain_size = params.size;
            let task = pool.spawn(async move {
                build_level_mesh(
                    coord,
                    target,
                    terrain_size,
                    &config,
                    height_source.as_ref(),
                    color_source.as_deref(),
                )
            });
            commands.entity(entity).insert(PendingChunkMesh {
                target_level: target,
                task,
            });
            builds_started += 1;
        }
    }

    let mut new_stats = TerrainLodStats {
        builds_last_frame: builds_started,
        measured_chunks: errors.measured_count() as u32,
        ..TerrainLodStats::default()
    };
    for (_, chunk, pending) in &chunks {
        let level = usize::from(chunk.level);
        new_stats.chunks_per_level[level] += 1;
        new_stats.vertices_total += chunk_vertex_count(lod.quads_per_chunk[level]) as u64;
        new_stats.pending_builds += u32::from(pending.is_some());
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
