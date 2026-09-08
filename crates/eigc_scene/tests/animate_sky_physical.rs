use bevy::MinimalPlugins;
use bevy::prelude::{App, Update};
use eigc_moons::{MoonId, MoonProfile, SkyCalibration, TerrainCalibration};
use eigc_scene::sky::{SkySettings, SkyState, animate_sky_physical};
use eigc_sim::SimTime;

/// Perfil de teste com sol e Júpiter perpendiculares entre si.
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
            jupiter_ang_radius: 0.104_72, // ~6 graus em radianos
            sun_elevation_deg: 20.0,
            eclipse_soft_deg: 1.0,
            planet_shine_max: 0.006,
        },
    }
}

/// Monta um App mínimo com `SkySettings`/`SkyState`/`SimTime`
fn app_with_sky_animation() -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .insert_resource(SkySettings::from_profile(&test_profile()))
        .init_resource::<SkyState>()
        .insert_resource(SimTime(0.0))
        .add_systems(Update, animate_sky_physical);
    app
}

/// Testa se as direções resultantes de sol e Júpiter permanecem normalizadas após a animação.
#[test]
fn animate_sky_physical_produces_normalized_directions() {
    let mut app = app_with_sky_animation();

    {
        let mut sim_time = app.world_mut().resource_mut::<SimTime>();
        sim_time.0 = 250.0;
    }
    app.update();

    let state = app.world().resource::<SkyState>();
    assert!(
        (state.sun_dir.length() - 1.0).abs() < 1e-5,
        "sun_dir não normalizado: {}",
        state.sun_dir.length()
    );
    assert!(
        (state.jupiter_dir.length() - 1.0).abs() < 1e-5,
        "jupiter_dir não normalizado: {}",
        state.jupiter_dir.length()
    );
}

/// Testa se o fator de eclipse é 1.0 quando o sol e Júpiter estão em posições angularmente distantes
#[test]
fn eclipse_factor_is_full_brightness_when_sun_far_from_jupiter() {
    let mut app = app_with_sky_animation();

    // não deveria haver eclipse
    app.update();

    let state = app.world().resource::<SkyState>();
    assert_eq!(
        state.eclipse_factor, 1.0,
        "esperava brilho total na configuração inicial do perfil de teste"
    );
}

/// Testa se o fator de brilho do planeta nunca excede o máximo configurado no perfil.
#[test]
fn planet_shine_factor_never_exceeds_configured_max() {
    let mut app = app_with_sky_animation();
    let orbital_period = 1000.0;

    for step in 0..20 {
        {
            let mut sim_time = app.world_mut().resource_mut::<SimTime>();
            sim_time.0 = (step as f32 / 20.0) * orbital_period;
        }

        app.update();

        let state = app.world().resource::<SkyState>();
        assert!(
            state.planet_shine_factor <= 0.006 + 1e-6,
            "planet_shine_factor excedeu o máximo configurado: {}",
            state.planet_shine_factor
        );
        assert!(
            state.planet_shine_factor >= 0.0,
            "planet_shine_factor é negativo: {}",
            state.planet_shine_factor
        );
    }
}

/// Testa se a função de animação do céu não altera a direção base do sol no `SkySettings`.
#[test]
fn animate_sky_physical_does_not_mutate_base_sun_dir_in_settings() {
    let mut app = app_with_sky_animation();
    let original_base_sun_dir = app.world().resource::<SkySettings>().base_sun_dir;

    {
        let mut sim_time = app.world_mut().resource_mut::<SimTime>();
        sim_time.0 = 500.0;
    }
    app.update();

    let settings = app.world().resource::<SkySettings>();
    assert_eq!(
        settings.base_sun_dir, original_base_sun_dir,
        "SkySettings::base_sun_dir não deveria mudar depois de animate_sky_physical, mas mudou de {:?} para {:?}",
        original_base_sun_dir, settings.base_sun_dir
    );
}
