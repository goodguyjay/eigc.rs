//! Regressão da volta ao menu: ao sair de `AppState::Running`, os chunks do terreno somem (soltando
//! malhas e material) e os resources da sessão são removidos, para a próxima lua não herdar nada.

use bevy::MinimalPlugins;
use bevy::prelude::{
    App, AssetApp, AssetPlugin, Assets, Handle, Mesh, NextState, StandardMaterial, Vec2,
};
use bevy::state::app::{AppExtStates, StatesPlugin};
use eigc_moons::profile::MoonProfile;
use eigc_moons::{ActiveMoonProfileHandle, AppState};
use eigc_terrain::chunks::{TerrainChunk, TerrainLodStats, spawn_terrain_chunks};
use eigc_terrain::height::{HeightSource, arc};
use eigc_terrain::lod::TerrainLodConfig;
use eigc_terrain::params::TerrainParams;
use eigc_terrain::pipeline::{HeightResource, TerrainPlugin};

/// Fonte de altura plana usada só para teste.
struct FlatHeight;

impl HeightSource for FlatHeight {
    fn height_at(&self, _x: f32, _z: f32) -> f32 {
        0.0
    }
}

/// Grade pequena de 2x2 chunks, só para existir o que limpar.
fn small_config() -> TerrainLodConfig {
    TerrainLodConfig {
        chunks_per_side: 2,
        quads_per_chunk: [8, 4, 2, 1],
        max_screen_error_px: 2.0,
        min_error_fraction: 0.01,
        hysteresis_fraction: 0.1,
        skirt_depth_factor: 0.35,
        in_flight_vertex_budget: 100_000,
        max_error_tasks_in_flight: 4,
    }
}

fn small_params() -> TerrainParams {
    TerrainParams {
        size: 800.0,
        res: 4,
        amp: 1.0,
        freq: 1.0,
        line_dir: Vec2::new(1.0, 0.0),
        seed: 0,
        vertical_exaggeration: 1.0,
    }
}

fn chunk_count(app: &mut App) -> usize {
    app.world_mut()
        .query::<&TerrainChunk>()
        .iter(app.world())
        .count()
}

/// App em `Running` com o `TerrainPlugin` e uma grade de chunks montada à mão, como
/// `build_terrain_from_loaded_profile` faria com um perfil carregado.
fn running_app_with_terrain() -> App {
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
    app.update();

    let params = small_params();
    let lod = small_config();
    let height = arc(FlatHeight);
    let world = app.world_mut();
    let material = world
        .resource_mut::<Assets<StandardMaterial>>()
        .add(StandardMaterial::default());
    world.resource_scope(|world, mut meshes: bevy::prelude::Mut<Assets<Mesh>>| {
        let mut commands = world.commands();
        spawn_terrain_chunks(
            &mut commands,
            &mut meshes,
            material,
            &params,
            &lod,
            height.as_ref(),
            None,
        );
    });
    world.flush();
    world.insert_resource(params);
    world.insert_resource(lod);
    world.insert_resource(HeightResource(height));

    for _ in 0..3 {
        app.update();
    }
    app
}

/// Testa que voltar ao menu despawna todos os chunks, solta as malhas e remove os resources do
/// terreno, zerando as estatísticas de LOD.
#[test]
fn leaving_running_despawns_chunks_and_removes_terrain_resources() {
    let mut app = running_app_with_terrain();
    assert_eq!(chunk_count(&mut app), 4);
    assert!(app.world().resource::<Assets<Mesh>>().len() > 0);

    app.world_mut()
        .resource_mut::<NextState<AppState>>()
        .set(AppState::MainMenu);
    app.update();
    app.update();

    assert_eq!(
        chunk_count(&mut app),
        0,
        "chunks sobreviveram fora de Running"
    );
    assert_eq!(
        app.world().resource::<Assets<Mesh>>().len(),
        0,
        "malhas dos chunks não foram liberadas"
    );
    assert!(!app.world().contains_resource::<HeightResource>());
    assert!(!app.world().contains_resource::<TerrainParams>());
    assert!(!app.world().contains_resource::<TerrainLodConfig>());
    assert_eq!(
        *app.world().resource::<TerrainLodStats>(),
        TerrainLodStats::default()
    );
}
