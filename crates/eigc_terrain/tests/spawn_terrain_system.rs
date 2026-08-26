//! Testes de integração para o sistema spawn_terrain.

use bevy::MinimalPlugins;
use bevy::prelude::{App, AssetApp, AssetPlugin, Color, Mesh, Name, StandardMaterial, Startup, Vec2};
use eigc_terrain::height::arc;
use eigc_terrain::params::TerrainParams;
use eigc_terrain::pipeline::{HeightResource, TerrainAppearance};
use eigc_terrain::systems::spawn_terrain;

/// Fonte de altura determinística usada só para teste.
struct FlatHeightSource;
impl eigc_terrain::height::HeightSource for FlatHeightSource {
    fn height_at(&self, _x: f32, _z: f32) -> f32 {
        0.0
    }
}

/// Testa se o sistema `spawn_terrain` cria exatamente uma entidade de terreno com o nome "Terrain".
#[test]
fn spawn_terrain_creates_exactly_one_terrain_entity() {
    let mut app = App::new();

    let terrain_params = TerrainParams {
        size: 10.0,
        res: 2,
        amp: 1.0,
        freq: 1.0,
        line_dir: Vec2::new(1.0, 0.0),
        seed: 0,
    };

    let terrain_appearance = TerrainAppearance {
        base_color: Color::WHITE,
        display_name: "Teste do terreno lunar".to_string(),
    };

    app.add_plugins(MinimalPlugins)
        .add_plugins(AssetPlugin::default())
        .init_asset::<Mesh>()
        .init_asset::<StandardMaterial>()
        .insert_resource(terrain_params)
        .insert_resource(HeightResource(arc(FlatHeightSource)))
        .insert_resource(terrain_appearance)
        .add_systems(Startup, spawn_terrain);

    app.update();

    let mut terrain_entities = app.world_mut().query::<&Name>();
    let matching_entity_count = terrain_entities
        .iter(app.world())
        .filter(|entity_name| entity_name.as_str() == "Teste do terreno lunar")
        .count();

    assert_eq!(
        matching_entity_count, 1,
        "spawn_terrain deveria criar exatamente uma entidade nomeada 'Teste do terreno lunar'"
    );
}
