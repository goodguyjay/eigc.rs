use bevy::asset::LoadState;
use bevy::prelude::{App, AssetPlugin, AssetServer, Assets, MinimalPlugins, default};
use eigc_moons::plugin::MoonPlugin;
use eigc_moons::profile::{MoonId, MoonProfile};
use std::io::Write;

/// Escreve um arquivo .ron temporário de teste em um diretório de assets
/// isolado, para não depender do conteúdo real de assets/moons/europa.ron.
fn write_temp_moon_profile(assets_dir: &std::path::Path, file_name: &str) {
    let content = r#"
MoonProfile(
    moon_id: Europa,
    display_name: "Profile de Teste",
    jupiter_angular_diameter_deg: 10.0,
    terrain: TerrainCalibration(
        seed: 1,
        base_frequency: 0.001,
        feature_direction: (1.0, 0.0),
        vertical_amplitude_meters: 5.0,
        warp_amplitude_meters: 10.0,
    ),
    terrain_base_color: (1.0, 1.0, 1.0, 1.0),
    walkable: true,
)
"#;
    std::fs::create_dir_all(assets_dir).unwrap();
    let mut file = std::fs::File::create(assets_dir.join(file_name)).unwrap();
    file.write_all(content.as_bytes()).unwrap();
}

/// Testa se o MoonProfile é carregado e desserializado corretamente pelo AssetServer do Bevy.
#[test]
fn moon_profile_loads_and_deserializes_correctly() {
    let temp_dir = std::env::temp_dir().join("eigc_moons_test_assets");
    write_temp_moon_profile(&temp_dir, "test_profile.ron");

    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .add_plugins(AssetPlugin {
            file_path: temp_dir.to_string_lossy().to_string(),
            ..default()
        })
        .add_plugins(MoonPlugin);

    let handle = {
        let asset_server = app.world().resource::<AssetServer>();
        asset_server.load::<MoonProfile>("test_profile.ron")
    };

    for _ in 0..1000 {
        app.update();

        let asset_server = app.world().resource::<AssetServer>();
        match asset_server.load_state(&handle) {
            LoadState::Loaded => {
                let profiles = app.world().resource::<Assets<MoonProfile>>();
                let profile = profiles
                    .get(&handle)
                    .expect("MoonProfile não encontrado após carregamento");

                assert_eq!(profile.moon_id, MoonId::Europa);
                assert_eq!(profile.display_name, "Profile de Teste");
                assert_eq!(profile.terrain.seed, 1);
                return;
            }

            LoadState::Failed(err) => {
                panic!("Falha ao carregar MoonProfile: {err}");
            }

            _ => { /* Ainda carregando, continua tentando */}
        }

        std::thread::sleep(std::time::Duration::from_millis(10));
    }

    panic!("MoonProfile não carregou dentro do limite de tentativas de update");
}
