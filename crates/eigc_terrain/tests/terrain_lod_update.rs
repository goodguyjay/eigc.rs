//! Testes de integração do LOD por chunks: spawn inicial, medição de erro e construção de malhas
//! em segundo plano, nível escolhido pelo erro em pixels, orçamento em voo e guarda do plugin
//! quando os resources ainda não existem.

use bevy::MinimalPlugins;
use bevy::prelude::{
    App, AssetApp, AssetPlugin, Assets, Commands, Handle, IntoScheduleConfigs, Mesh, Mesh3d,
    NextState, Res, ResMut, StandardMaterial, Startup, Update, Vec2, Vec3,
};
use bevy::state::app::{AppExtStates, StatesPlugin};
use eigc_common::lod_focus::LodFocus;
use eigc_moons::profile::MoonProfile;
use eigc_moons::{ActiveMoonProfileHandle, AppState};
use eigc_terrain::chunks::{
    ChunkErrorTable, TerrainChunk, TerrainLodStats, finish_chunk_builds, measure_chunk_errors,
    spawn_terrain_chunks, update_terrain_lod,
};
use eigc_terrain::height::{HeightSource, arc};
use eigc_terrain::lod::{ChunkCoord, LOD_LEVEL_COUNT, TerrainLodConfig};
use eigc_terrain::params::TerrainParams;
use eigc_terrain::pipeline::{HeightResource, TerrainPlugin};
use std::time::Duration;

const TERRAIN_SIZE: f32 = 1600.0;
const COARSEST_LEVEL: u8 = (LOD_LEVEL_COUNT - 1) as u8;
const CHUNK_COUNT: usize = 16;

/// Fonte de altura plana usada só para teste.
struct FlatHeight;

impl HeightSource for FlatHeight {
    fn height_at(&self, _x: f32, _z: f32) -> f32 {
        0.0
    }
}

/// Metade -X lisa e metade +X muito irregular, para comparar chunks à mesma distância.
struct SmoothAndJaggedHeight;

impl HeightSource for SmoothAndJaggedHeight {
    fn height_at(&self, x: f32, z: f32) -> f32 {
        if x < 0.0 {
            0.0
        } else {
            ((x * 0.9).sin() + (z * 1.3).cos()) * 4.0
        }
    }
}

/// Grade de 4x4 chunks de 400 m com quads pequenos, para os testes serem rápidos.
///
/// Com relevo plano o erro medido é zero e vale o piso: com `screen_scale` 1000, tolerância de
/// 2 px e piso de 1% do espaçamento, os limites de distância ficam em 500, 1000 e 2000 m.
fn test_config(in_flight_vertex_budget: u32) -> TerrainLodConfig {
    TerrainLodConfig {
        chunks_per_side: 4,
        quads_per_chunk: [8, 4, 2, 1],
        max_screen_error_px: 2.0,
        min_error_fraction: 0.01,
        hysteresis_fraction: 0.1,
        skirt_depth_factor: 0.35,
        in_flight_vertex_budget,
        max_error_tasks_in_flight: 16,
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

/// Monta um app com os chunks já spawnados e os sistemas de LOD rodando em `Update`.
fn lod_test_app(
    height: impl HeightSource,
    focus: Vec3,
    screen_scale: f32,
    in_flight_vertex_budget: u32,
) -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .add_plugins(AssetPlugin::default())
        .init_asset::<Mesh>()
        .init_asset::<StandardMaterial>()
        .insert_resource(LodFocus {
            position: focus,
            screen_scale,
        })
        .insert_resource(test_params())
        .insert_resource(test_config(in_flight_vertex_budget))
        .insert_resource(HeightResource(arc(height)))
        .init_resource::<TerrainLodStats>()
        .init_resource::<ChunkErrorTable>()
        .add_systems(Startup, spawn_test_chunks)
        .add_systems(
            Update,
            (measure_chunk_errors, finish_chunk_builds, update_terrain_lod).chain(),
        );
    app.update();
    app
}

/// Roda frames, dormindo um pouco entre eles para as tarefas em segundo plano andarem, até a
/// condição valer. Falha o teste se passar de 5000 frames.
fn run_until(app: &mut App, mut condition: impl FnMut(&App) -> bool) {
    for _ in 0..5000 {
        if condition(app) {
            return;
        }
        app.update();
        std::thread::sleep(Duration::from_millis(1));
    }
    panic!("condição não foi atingida em 5000 frames");
}

/// Roda até todos os chunks estarem medidos e nenhuma construção estar pendente nem começando,
/// por 3 frames seguidos.
fn settle(app: &mut App) {
    let mut quiet_frames = 0;
    run_until(app, |app| {
        let stats = app.world().resource::<TerrainLodStats>();
        let quiet = stats.measured_chunks as usize == CHUNK_COUNT
            && stats.pending_builds == 0
            && stats.builds_last_frame == 0;
        quiet_frames = if quiet { quiet_frames + 1 } else { 0 };
        quiet_frames >= 3
    });
}

/// Lista `(chunk, vértices da malha atual)` de todos os chunks.
fn chunk_snapshot(app: &mut App) -> Vec<(TerrainChunk, usize)> {
    let mut query = app.world_mut().query::<(&TerrainChunk, &Mesh3d)>();
    let meshes = app.world().resource::<Assets<Mesh>>();
    query
        .iter(app.world())
        .map(|(chunk, mesh)| {
            let vertices = meshes
                .get(&mesh.0)
                .expect("malha do chunk não está em Assets<Mesh>")
                .count_vertices();
            (*chunk, vertices)
        })
        .collect()
}

fn level_of(app: &mut App, coord: ChunkCoord) -> u8 {
    chunk_snapshot(app)
        .into_iter()
        .find(|(chunk, _)| chunk.coord == coord)
        .map(|(chunk, _)| chunk.level)
        .expect("chunk não encontrado")
}

/// A grade tem um chunk por célula e o sistema de LOD nunca cria nem remove chunks.
#[test]
fn lod_update_never_spawns_or_despawns_chunks() {
    let mut app = lod_test_app(FlatHeight, Vec3::new(0.0, 100.0, 0.0), 1000.0, 100_000);
    assert_eq!(chunk_snapshot(&mut app).len(), CHUNK_COUNT);

    for step in 0..6 {
        app.world_mut().resource_mut::<LodFocus>().position =
            Vec3::new(step as f32 * 400.0 - 1000.0, 100.0, 300.0);
        for _ in 0..5 {
            app.update();
            std::thread::sleep(Duration::from_millis(1));
        }
        assert_eq!(chunk_snapshot(&mut app).len(), CHUNK_COUNT);
    }
}

/// Chunks ficam sem medição (e no nível mais grosso) até a medição em segundo plano chegar.
#[test]
fn chunks_are_measured_in_the_background_and_all_end_up_measured() {
    let mut app = lod_test_app(FlatHeight, Vec3::ZERO, 1000.0, 100_000);

    settle(&mut app);

    let stats = app.world().resource::<TerrainLodStats>();
    assert_eq!(stats.measured_chunks as usize, CHUNK_COUNT);
    assert_eq!(app.world().resource::<ChunkErrorTable>().measured_count(), CHUNK_COUNT);
}

/// Chunk sob o foco fica no nível mais fino, com a malha correspondente; o mais distante fica
/// em um nível mais grosso.
#[test]
fn chunk_under_focus_is_refined_and_far_chunk_stays_coarser() {
    let near = ChunkCoord { ix: 0, iz: 0 };
    let far = ChunkCoord { ix: 3, iz: 3 };
    let mut app = lod_test_app(FlatHeight, Vec3::new(-600.0, 0.0, -600.0), 1000.0, 100_000);

    settle(&mut app);

    assert_eq!(level_of(&mut app, near), 0);
    assert!(level_of(&mut app, far) > 0);

    let near_vertices = chunk_snapshot(&mut app)
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
    let mut app = lod_test_app(FlatHeight, Vec3::new(-600.0, 0.0, -600.0), 1000.0, 100_000);
    settle(&mut app);
    assert_eq!(level_of(&mut app, near), 0);

    app.world_mut().resource_mut::<LodFocus>().position = Vec3::new(50_000.0, 0.0, 50_000.0);
    settle(&mut app);
    assert_eq!(level_of(&mut app, near), COARSEST_LEVEL);
}

/// Dobrar a escala de tela (janela maior ou FOV menor) leva o mesmo chunk a um nível mais fino.
#[test]
fn larger_screen_scale_makes_the_same_chunk_finer() {
    let probe = ChunkCoord { ix: 3, iz: 3 };
    let focus = Vec3::new(-600.0, 0.0, -600.0);

    let mut low = lod_test_app(FlatHeight, focus, 250.0, 100_000);
    settle(&mut low);
    let mut high = lod_test_app(FlatHeight, focus, 4000.0, 100_000);
    settle(&mut high);

    assert!(level_of(&mut high, probe) < level_of(&mut low, probe));
}

/// O ponto da abordagem: à mesma distância, o chunk irregular fica mais fino que o liso.
#[test]
fn rough_chunk_is_refined_from_farther_away_than_a_smooth_chunk() {
    let smooth = ChunkCoord { ix: 0, iz: 1 };
    let rough = ChunkCoord { ix: 3, iz: 1 };
    // Escala de tela baixa: o piso do chunk liso dá limites de 100, 200 e 400 m, então a 400 m
    // ele já pode ficar no nível mais grosso. O erro medido do chunk irregular (vários metros)
    // empurra os limites dele para além de 400 m.
    let mut app = lod_test_app(SmoothAndJaggedHeight, Vec3::new(0.0, 0.0, 0.0), 200.0, 100_000);

    settle(&mut app);

    assert_eq!(level_of(&mut app, rough), 0);
    assert_eq!(level_of(&mut app, smooth), COARSEST_LEVEL);
}

/// Com orçamento de um chunk fino, nunca há mais de uma construção em voo, e ainda assim todos
/// acabam refinados.
#[test]
fn in_flight_budget_limits_concurrent_builds_but_everything_still_converges() {
    let budget = vertices_with_skirt(8) as u32;
    // Escala de tela enorme: todo chunk quer o nível 0.
    let mut app = lod_test_app(FlatHeight, Vec3::ZERO, 1.0e7, budget);

    let mut worst_in_flight = 0;
    run_until(&mut app, |app| {
        let stats = app.world().resource::<TerrainLodStats>();
        worst_in_flight = worst_in_flight.max(stats.pending_builds + stats.builds_last_frame);
        stats.chunks_per_level[0] as usize == CHUNK_COUNT
    });

    assert!(worst_in_flight <= 1, "construções em voo: {worst_in_flight}");
}

/// As estatísticas somam todos os chunks e batem com as malhas.
#[test]
fn lod_stats_match_chunk_levels_and_mesh_vertices() {
    let mut app = lod_test_app(FlatHeight, Vec3::new(-600.0, 0.0, -600.0), 1000.0, 100_000);
    settle(&mut app);

    let stats = app.world().resource::<TerrainLodStats>().clone();
    assert_eq!(stats.chunks_per_level.iter().sum::<u32>() as usize, CHUNK_COUNT);

    let expected_vertices: usize = chunk_snapshot(&mut app)
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

    assert!(chunk_snapshot(&mut app).is_empty());
}
