//! Regressão para o risco arquitetural de limite de `DirectionalLight` documentado no
//! `CLAUDE.md` deste crate: monta o mesmo pipeline de plugins que `eigc_app::main` usa em
//! produção (menos janela/renderer) e conta quantas `DirectionalLight`s existem depois do
//! céu terminar de carregar.

use bevy::MinimalPlugins;
use bevy::asset::AssetApp;
use bevy::image::{CompressedImageFormats, ImageLoader, ImagePlugin};
use bevy::prelude::{
    App, AppExtStates, AssetPlugin, Assets, DirectionalLight, Mesh, NextState, StandardMaterial,
    default,
};
use bevy::state::app::StatesPlugin;
use eigc_moons::{
    ActiveMoonProfileHandle, AppState, MoonId, MoonPlugin, MoonProfile, SkyCalibration,
    TerrainCalibration,
};
use eigc_scene::sky::{SkyAssets, SkyPlugin};
use eigc_sim::SimTime;

/// Limite real de `DirectionalLight`s simultâneas suportado pelo Bevy 0.18
/// (`bevy_pbr::render::light::MAX_DIRECTIONAL_LIGHTS`).
const BEVY_MAX_DIRECTIONAL_LIGHTS: usize = 10;

/// Perfil de teste com valores arbitrários, suficiente para calibrar `SkySettings`.
fn test_profile() -> MoonProfile {
    MoonProfile {
        moon_id: MoonId::Europa,
        display_name: "Perfil de teste".to_string(),
        jupiter_angular_diameter_deg: 12.0,
        terrain: TerrainCalibration {
            seed: 1,
            base_frequency: 0.001,
            feature_direction: [1.0, 0.0],
            vertical_amplitude_meters: 10.0,
            warp_amplitude_meters: 20.0,
            perceptual_roughness: 0.5,
            reflectance: 0.3,
        },
        terrain_base_color: [1.0, 1.0, 1.0, 1.0],
        walkable: true,
        sky: SkyCalibration {
            orbital_period_seconds: 1000.0,
            base_sun_dir: [0.0, 0.3, -1.0],
            base_jupiter_dir: [1.0, 0.2, 0.0],
            jupiter_libration_lat_deg: 0.0,
            jupiter_libration_lon_deg: 0.0,
            jupiter_ang_radius: 0.104_72,
            sun_elevation_deg: 20.0,
            eclipse_soft_deg: 1.0,
            planet_shine_max: 0.006,
        },
    }
}

/// Monta o App headless com o mesmo conjunto de plugins de céu/lua que `eigc_app::main` usa,
/// sem janela nem renderer, e transiciona direto para `AppState::Running`.
fn app_with_sky_running() -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .add_plugins(AssetPlugin {
            file_path: "../../assets".to_string(),
            ..default()
        })
        .add_plugins(ImagePlugin::default())
        .register_asset_loader(ImageLoader::new(CompressedImageFormats::NONE))
        .init_asset::<Mesh>()
        .init_asset::<StandardMaterial>()
        .add_plugins(StatesPlugin)
        .init_state::<AppState>()
        .add_plugins(MoonPlugin)
        .add_plugins(SkyPlugin)
        .insert_resource(SimTime(0.0));

    let handle = {
        let mut profiles = app.world_mut().resource_mut::<Assets<MoonProfile>>();
        profiles.add(test_profile())
    };
    app.insert_resource(ActiveMoonProfileHandle(handle));
    app.world_mut()
        .resource_mut::<NextState<AppState>>()
        .set(AppState::Running);

    app
}

/// Conta quantas entidades `DirectionalLight` existem no mundo do app.
fn count_directional_lights(app: &mut App) -> usize {
    let mut query = app.world_mut().query::<&DirectionalLight>();
    query.iter(app.world()).count()
}

/// Testa que o número de `DirectionalLight`s spawnadas pelo pipeline de produção (céu +
/// planetshine) fica dentro do limite suportado pelo Bevy. Esse teste é a guarda de regressão
/// pedida no `BACKLOG.md`: se alguém somar mais uma luz permanente (ex.: reintroduzir um
/// placeholder esquecido), o teste quebra antes do commit passar.
#[test]
fn directional_light_count_stays_within_bevy_limit_after_sky_loads() {
    let mut app = app_with_sky_running();

    let mut sky_assets_loaded = false;
    for _ in 0..1000 {
        app.update();

        if let Some(assets) = app.world().get_resource::<SkyAssets>()
            && assets.all_loaded
        {
            sky_assets_loaded = true;
            // mais um update para o SunLight ser spawnado a partir da mensagem SkyAssetsLoaded.
            app.update();
            break;
        }

        std::thread::sleep(std::time::Duration::from_millis(5));
    }

    assert!(
        sky_assets_loaded,
        "texturas do céu não terminaram de carregar dentro do limite de tentativas"
    );

    let count = count_directional_lights(&mut app);
    assert!(
        count <= BEVY_MAX_DIRECTIONAL_LIGHTS,
        "{count} DirectionalLight(s) simultâneas excede o limite de {BEVY_MAX_DIRECTIONAL_LIGHTS} suportado pelo Bevy"
    );

    // hoje só existem duas: SunLight e PlanetShine. Fixar o número exato (em vez de só checar
    // o teto do Bevy) é o que realmente pega regressão, tipo o placeholder vestigial que existia
    // em `eigc_app::scene_placeholder` antes desse teste existir.
    assert_eq!(
        count, 2,
        "esperava exatamente 2 DirectionalLight(s) (SunLight + PlanetShine) depois do céu carregar, achei {count}. \
         Se essa mudança foi intencional, atualize esse número e confirme que ainda está dentro do limite do Bevy."
    );
}
