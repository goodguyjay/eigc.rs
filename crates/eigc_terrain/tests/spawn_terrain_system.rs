//! Testes de integração para o sistema spawn_terrain.

use bevy::MinimalPlugins;
use bevy::prelude::{
    App, AssetApp, AssetPlugin, Assets, Color, Mesh, MeshMaterial3d, Name, StandardMaterial,
    Startup, Vec2,
};
use eigc_terrain::height::arc;
use eigc_terrain::params::TerrainParams;
use eigc_terrain::pipeline::{HeightResource, TerrainAppearance};
use eigc_terrain::systems::{TerrainMaterialProperties, spawn_terrain};

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

    let material_properties = TerrainMaterialProperties {
        perceptual_roughness: 0.5,
        reflectance: 0.3,
    };

    app.add_plugins(MinimalPlugins)
        .add_plugins(AssetPlugin::default())
        .init_asset::<Mesh>()
        .init_asset::<StandardMaterial>()
        .insert_resource(terrain_params)
        .insert_resource(HeightResource(arc(FlatHeightSource)))
        .insert_resource(terrain_appearance)
        .insert_resource(material_properties)
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

/// Testa que spawn_terrain aplica perceptual_roughness e reflectance vindos de
/// TerrainMaterialProperties
#[test]
fn spawn_terrain_applies_material_properties_from_resource() {
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

    let material_properties = TerrainMaterialProperties {
        perceptual_roughness: 0.42,
        reflectance: 0.13,
    };

    app.add_plugins(MinimalPlugins)
        .add_plugins(AssetPlugin::default())
        .init_asset::<Mesh>()
        .init_asset::<StandardMaterial>()
        .insert_resource(terrain_params)
        .insert_resource(HeightResource(arc(FlatHeightSource)))
        .insert_resource(terrain_appearance)
        .insert_resource(material_properties)
        .add_systems(Startup, spawn_terrain);

    app.update();

    let mut terrain_query = app
        .world_mut()
        .query::<(&Name, &MeshMaterial3d<StandardMaterial>)>();

    let (_, material_handle) = terrain_query
        .iter(app.world())
        .find(|(name, _)| name.as_str() == "Teste do terreno lunar")
        .expect("Entidade de terreno não encontrada");

    let materials = app.world().resource::<Assets<StandardMaterial>>();
    let material = materials
        .get(&material_handle.0)
        .expect("Material do terreno não encontrado em Assets");

    assert_eq!(
        material.perceptual_roughness, 0.42,
        "perceptual_roughness do material não corresponde ao valor esperado"
    );
    assert_eq!(
        material.reflectance, 0.13,
        "reflectance do material não corresponde ao valor esperado"
    )
}
