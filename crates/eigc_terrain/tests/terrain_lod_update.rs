//! Testes de integração do LOD por chunks: spawn inicial, troca de nível pela distância ao foco,
//! orçamento de vértices por frame e guarda do plugin quando os resources ainda não existem.

use bevy::MinimalPlugins;
use bevy::prelude::{
    App, AssetApp, AssetPlugin, Assets, Commands, Handle, Mesh, NextState, Res, ResMut,
    StandardMaterial, Startup, Update, Vec2, Vec3,
};
use bevy::state::app::{AppExtStates, StatesPlugin};
use eigc_common::lod_focus::LodFocus;
use eigc_moons::profile::MoonProfile;
use eigc_moons::{ActiveMoonProfileHandle, AppState};
use eigc_terrain::chunks::{TerrainChunk, TerrainLodStats, spawn_terrain_chunks, update_terrain_lod};
use eigc_terrain::height::{HeightSource, arc};
use eigc_terrain::lod::{ChunkCoord, LOD_LEVEL_COUNT, TerrainLodConfig};
use eigc_terrain::params::TerrainParams;
use eigc_terrain::pipeline::{HeightResource, TerrainPlugin};

const TERRAIN_SIZE: f32 = 1600.0;
const COARSEST_LEVEL: u8 = (LOD_LEVEL_COUNT - 1) as u8;

/// Fonte de altura plana usada só para teste.
struct FlatHeight;

impl HeightSource for FlatHeight {
    fn height_at(&self, _x: f32, _z: f32) -> f32 {
        0.0
    }
}

/// Grade de 4x4 chunks de 400 m com quads pequenos, para os testes serem rápidos.
fn test_config(vertex_budget_per_frame: u32) -> TerrainLodConfig {
    TerrainLodConfig {
        chunks_per_side: 4,
        quads_per_chunk: [8, 4, 2, 1],
        distance_thresholds: [500.0, 1000.0, 2000.0],
        hysteresis_fraction: 0.1,
        skirt_depth_factor: 0.35,
        vertex_budget_per_frame,
    }
}

fn test_params() -> TerrainParams {
    TerrainParams {
        size: TERRAIN_SIZE,
        res: 4,
        amp: 1.0,
        freq: 1.0,
        line_dir: Vec2::new(1.0, 0.0),
        seed: 0,
    }
}

/// Número de vértices de um chunk com skirt: grade `(q+1)^2` mais 4 bordas de `q+1`.
fn vertices_with_skirt(quads: u32) -> usize {
    let side = quads as usize + 1;
    side * side + 4 * side
}

/// Sistema de teste que spawna a grade de chunks no `Startup`, como o plugin faz no `OnEnter`.
fn spawn_test_chunks(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    params: Res<TerrainParams>,
    lod: Res<TerrainLodConfig>,
    height: Res<HeightResource>,
) {
    let material: Handle<StandardMaterial> = materials.add(StandardMaterial::default());
    spawn_terrain_chunks(
        &mut commands,
        &mut meshes,
        material,
        &params,
        &lod,
        height.0.as_ref(),
        None,
    );
}

/// Monta um app com os chunks já spawnados e o sistema de LOD rodando em `Update`.
fn lod_test_app(focus: Vec3, vertex_budget_per_frame: u32) -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .add_plugins(AssetPlugin::default())
        .init_asset::<Mesh>()
        .init_asset::<StandardMaterial>()
        .insert_resource(LodFocus { position: focus })
        .insert_resource(test_params())
        .insert_resource(test_config(vertex_budget_per_frame))
        .insert_resource(HeightResource(arc(FlatHeight)))
        .init_resource::<TerrainLodStats>()
        .add_systems(Startup, spawn_test_chunks)
        .add_systems(Update, update_terrain_lod);
    app.update();
    app
}

fn all_chunks(app: &mut App) -> Vec<(TerrainChunk, usize)> {
    let mut query = app.world_mut().query::<&TerrainChunk>();
    let chunks: Vec<TerrainChunk> = query.iter(app.world()).copied().collect();
    chunks
        .into_iter()
        .map(|chunk| {
            let vertex_count = mesh_vertex_count(app, chunk);
            (chunk, vertex_count)
        })
        .collect()
}

fn mesh_vertex_count(app: &mut App, chunk: TerrainChunk) -> usize {
    let mut query = app
        .world_mut()
        .query::<(&TerrainChunk, &bevy::prelude::Mesh3d)>();
    let handle = query
        .iter(app.world())
        .find(|(candidate, _)| candidate.coord == chunk.coord)
        .map(|(_, mesh)| mesh.0.clone())
        .expect("chunk sem Mesh3d");
    app.world()
        .resource::<Assets<Mesh>>()
        .get(&handle)
        .expect("malha do chunk não está em Assets<Mesh>")
        .count_vertices()
}

fn level_of(app: &mut App, coord: ChunkCoord) -> u8 {
    all_chunks(app)
        .into_iter()
        .find(|(chunk, _)| chunk.coord == coord)
        .map(|(chunk, _)| chunk.level)
        .expect("chunk não encontrado")
}

/// A grade tem um chunk por célula e o sistema de LOD nunca cria nem remove chunks.
#[test]
fn lod_update_never_spawns_or_despawns_chunks() {
    let mut app = lod_test_app(Vec3::new(0.0, 100.0, 0.0), 100_000);
    assert_eq!(all_chunks(&mut app).len(), 16);

    for step in 0..6 {
        app.world_mut().resource_mut::<LodFocus>().position =
            Vec3::new(step as f32 * 400.0 - 1000.0, 100.0, 300.0);
        app.update();
        assert_eq!(all_chunks(&mut app).len(), 16);
    }
}

/// Chunk sob o foco fica no nível mais fino, com a malha correspondente; o mais distante fica
/// em um nível mais grosso.
#[test]
fn chunk_under_focus_is_refined_and_far_chunk_stays_coarser() {
    let near = ChunkCoord { ix: 0, iz: 0 };
    let far = ChunkCoord { ix: 3, iz: 3 };
    let mut app = lod_test_app(Vec3::new(-600.0, 0.0, -600.0), 100_000);
    app.update();

    assert_eq!(level_of(&mut app, near), 0);
    assert!(level_of(&mut app, far) > 0);

    let near_vertices = all_chunks(&mut app)
        .into_iter()
        .find(|(chunk, _)| chunk.coord == near)
        .map(|(_, vertices)| vertices)
        .expect("chunk não encontrado");
    assert_eq!(near_vertices, vertices_with_skirt(8));
}

/// Afastar o foco devolve os chunks ao nível mais grosso.
#[test]
fn moving_focus_away_returns_chunks_to_the_coarsest_level() {
    let near = ChunkCoord { ix: 0, iz: 0 };
    let mut app = lod_test_app(Vec3::new(-600.0, 0.0, -600.0), 100_000);
    app.update();
    assert_eq!(level_of(&mut app, near), 0);

    app.world_mut().resource_mut::<LodFocus>().position = Vec3::new(50_000.0, 0.0, 50_000.0);
    app.update();
    assert_eq!(level_of(&mut app, near), COARSEST_LEVEL);
}

/// Com orçamento de um chunk fino por frame, só um chunk é refinado por update.
#[test]
fn vertex_budget_limits_how_many_chunks_change_per_frame() {
    let budget = vertices_with_skirt(8) as u32;
    let mut app = lod_test_app(Vec3::ZERO, budget);

    // O primeiro update do helper já rodou o sistema uma vez.
    let refined_after_first_update = all_chunks(&mut app)
        .iter()
        .filter(|(chunk, _)| chunk.level != COARSEST_LEVEL)
        .count();
    assert_eq!(refined_after_first_update, 1);
    assert_eq!(app.world().resource::<TerrainLodStats>().builds_last_frame, 1);

    app.update();
    let refined_after_second_update = all_chunks(&mut app)
        .iter()
        .filter(|(chunk, _)| chunk.level != COARSEST_LEVEL)
        .count();
    assert_eq!(refined_after_second_update, 2);
}

/// As estatísticas somam todos os chunks e batem com as malhas.
#[test]
fn lod_stats_match_chunk_levels_and_mesh_vertices() {
    let mut app = lod_test_app(Vec3::new(-600.0, 0.0, -600.0), 100_000);
    app.update();

    let stats = app.world().resource::<TerrainLodStats>().clone();
    assert_eq!(stats.chunks_per_level.iter().sum::<u32>(), 16);

    let expected_vertices: usize = all_chunks(&mut app)
        .iter()
        .map(|(_, vertices)| *vertices)
        .sum();
    assert_eq!(stats.vertices_total, expected_vertices as u64);
}

/// Em `Running`, sem terem sido inseridos os resources do terreno (perfil ainda não carregado),
/// o plugin não entra em pânico nem cria chunks.
#[test]
fn plugin_does_not_panic_in_running_state_before_terrain_resources_exist() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .add_plugins(AssetPlugin::default())
        .add_plugins(StatesPlugin)
        .init_asset::<Mesh>()
        .init_asset::<StandardMaterial>()
        .init_asset::<MoonProfile>()
        .init_state::<AppState>()
        .insert_resource(ActiveMoonProfileHandle(Handle::default()))
        .add_plugins(TerrainPlugin);

    app.world_mut()
        .resource_mut::<NextState<AppState>>()
        .set(AppState::Running);
    for _ in 0..3 {
        app.update();
    }

    assert!(all_chunks(&mut app).is_empty());
}
