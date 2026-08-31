//! Plugin de céu: Júpiter, planet shine e starfield, calibrados por lua ativa.

use bevy::app::App;
use bevy::prelude::{
    AssetServer, Assets, Commands, Handle, Image, IntoScheduleConfigs, Message, MessageWriter,
    OnEnter, Plugin, Quat, Res, ResMut, Resource, Update, Vec3, error, in_state,
};
use eigc_moons::{ActiveMoonProfileHandle, AppState, MoonProfile};
use eigc_sim::{SimSet, SimTime};

mod jupiter;
mod planet_shine;
mod shared;
mod starfield;
mod sun;

/// Texturas do céu, carregadas no início da simulação.
#[derive(Resource)]
pub struct SkyAssets {
    pub jupiter_tex: Handle<Image>,
    pub sun_tex: Handle<Image>,
    pub starfield_tex: Handle<Image>,
    pub all_loaded: bool,
}

/// Disparado uma única vez quando as três texturas de céu terminam de carregar.
#[derive(Default, Message)]
pub struct SkyAssetsLoaded;

/// Plugin de céu: Júpiter, planet shine e starfield, calibrados por lua ativa.
pub struct SkyPlugin;

/// Direções e fatores atuais do céu, recalculados todo frame por `animate_sky_physical`
#[derive(Resource, Default, Clone, Copy)]
pub struct SkyState {
    /// Fator de eclipse, 0.0 = eclipse total, 1.0 = sem eclipse
    pub eclipse_factor: f32,
    /// Fator da reflectância de Jupiter, 0.0 = sem planet shine, 1.0 = planet shine máximo
    pub planet_shine_factor: f32,
    /// Direção do sol, normalizada
    pub sun_dir: Vec3,
    /// Direção de Júpiter, normalizada
    pub jupiter_dir: Vec3,
}

/// Configurações físicas do céu, calibradas para a lua ativa.
#[derive(Resource, Clone)]
pub struct SkySettings {
    /// Direção base de Júpiter
    pub base_jupiter_dir: Vec3,
    /// Direção base do Sol
    pub base_sun_dir: Vec3,
    /// Iluminância do sol, em lux
    pub sun_illuminance: f32,
    /// Brilho ambiente, em lux
    pub ambient_brightness: f32,
    /// Período orbital, em segundos
    pub orbital_period_seconds: f32,
    /// Normal do plano orbital
    pub orbit_normal: Vec3,
    /// Latitude de libração de Júpiter, em radianos
    pub jupiter_libration_lat: f32,
    /// Longitude de libração de Júpiter, em radianos
    pub jupiter_libration_lon: f32,
    /// Raio angular do sol, em radianos
    pub sun_ang_radius: f32,
    /// Raio angular de Júpiter, em radianos
    pub jupiter_ang_radius: f32,
    /// Máximo de reflectância de Júpiter, em lux
    pub planet_shine_max: f32,
    /// Suavização do eclipse, em radianos
    pub eclipse_soft: f32,
    /// Elevação do sol, em radianos
    pub sun_elevation: f32,
}

/// Constrói `SkySettings` a partir do `MoonProfile` ativo.
impl SkySettings {
    /// Constrói a calibração de céu a partir de `MoonProfile` ativo.
    pub fn from_profile(profile: &MoonProfile) -> Self {
        let sky = &profile.sky;
        Self {
            base_jupiter_dir: Vec3::from_array(sky.base_jupiter_dir).normalize(),
            base_sun_dir: Vec3::from_array(sky.base_sun_dir).normalize(),
            sun_illuminance: 4500.0, // sol a ~5.2 UA, universal entre as quatro luas
            ambient_brightness: 0.05,
            orbital_period_seconds: sky.orbital_period_seconds,
            orbit_normal: Vec3::Y,
            jupiter_libration_lat: sky.jupiter_libration_lat_deg.to_radians(),
            jupiter_libration_lon: sky.jupiter_libration_lon_deg.to_radians(),
            sun_ang_radius: (0.53_f32 / 5.2).to_radians() * 0.5, // ~0.5° / 5.2 UA universal
            jupiter_ang_radius: sky.jupiter_ang_radius,
            planet_shine_max: sky.planet_shine_max,
            eclipse_soft: sky.eclipse_soft_deg.to_radians(),
            sun_elevation: sky.sun_elevation_deg.to_radians(),
        }
    }
}

/// Plugin de céu: Júpiter, planet shine e starfield, calibrados por lua ativa.
impl Plugin for SkyPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<SkyAssetsLoaded>()
            .add_systems(
                OnEnter(AppState::Running),
                (build_sky_settings, load_sky_assets),
            )
            .add_systems(
                Update,
                check_sky_assets_loaded.run_if(in_state(AppState::Running)),
            )
            .init_resource::<SkyState>()
            .add_plugins((
                sun::SunPlugin,
                jupiter::JupiterPlugin,
                starfield::StarfieldPlugin,
                planet_shine::PlanetShinePlugin,
            ))
            .add_systems(Update, animate_sky_physical.in_set(SimSet::Animate));
    }
}

/// Constrói `SkySettings` a partir do `MoonProfile` ativo, e adiciona como recurso.
fn build_sky_settings(
    mut commands: Commands,
    active: Res<ActiveMoonProfileHandle>,
    profiles: Res<Assets<MoonProfile>>,
) {
    let Some(profile) = profiles.get(&active.0) else {
        error!("SkyPlugin rodou sem MoonProfile carregado.");
        return;
    };

    commands.insert_resource(SkySettings::from_profile(profile));
}

/// Carrega as texturas de céu e adiciona `SkyAssets` como recurso.
fn load_sky_assets(mut commands: Commands, asset_server: Res<AssetServer>) {
    let jupiter_tex: Handle<Image> = asset_server.load("sky/Jupiter.jpg");
    let sun_tex: Handle<Image> = asset_server.load("sky/Sun.jpg");
    let starfield_tex: Handle<Image> = asset_server.load("sky/Starfield.jpg");

    commands.insert_resource(SkyAssets {
        jupiter_tex,
        sun_tex,
        starfield_tex,
        all_loaded: false,
    });
}

/// Verifica se todas as texturas de céu terminaram de carregar, e dispara `SkyAssetsLoaded` quando sim.
fn check_sky_assets_loaded(
    assets: Option<ResMut<SkyAssets>>,
    asset_server: Res<AssetServer>,
    mut loaded_events: MessageWriter<SkyAssetsLoaded>,
) {
    let Some(mut assets) = assets else {
        // não rodou load_sky_assets ainda, rola por um frame ou dois, sei lá.
        return;
    };

    if assets.all_loaded {
        return;
    }

    let jupiter_loaded = asset_server.is_loaded_with_dependencies(&assets.jupiter_tex);
    let sun_loaded = asset_server.is_loaded_with_dependencies(&assets.sun_tex);
    let starfield_loaded = asset_server.is_loaded_with_dependencies(&assets.starfield_tex);

    if jupiter_loaded && sun_loaded && starfield_loaded {
        assets.all_loaded = true;
        loaded_events.write(SkyAssetsLoaded);
    }
}

/// Atualiza a direção do sol, direção de Júpiter, fator de eclipse e fator de planet shine.
pub fn animate_sky_physical(
    settings: Res<SkySettings>,
    mut state: ResMut<SkyState>,
    sim: Res<SimTime>,
) {
    let t = sim.0;
    let wob_lat = settings.jupiter_libration_lat
        * (0.3 * t / settings.orbital_period_seconds * 2.0 * std::f32::consts::PI).sin();
    let wob_lon = settings.jupiter_libration_lon
        * (0.2 * t / settings.orbital_period_seconds * 2.0 * std::f32::consts::PI).sin();

    let up = settings.orbit_normal.normalize();
    let right = settings.base_jupiter_dir.normalize().cross(up).normalize();
    let jup_wobble = Quat::from_axis_angle(right, wob_lat) * Quat::from_axis_angle(up, wob_lon);
    let jupiter_dir = (jup_wobble * settings.base_jupiter_dir).normalize();

    let phase = (t / settings.orbital_period_seconds) * 2.0 * std::f32::consts::PI;
    let rot = Quat::from_axis_angle(up, phase);
    let horizon_dir = (-jupiter_dir).reject_from(up).normalize();
    let elevation = settings.sun_elevation;
    let noon = (horizon_dir * elevation.cos() + up * elevation.sin()).normalize();
    let sun_dir = (rot * noon).normalize();

    let sep = sun_dir.angle_between(-jupiter_dir);
    let penumbra = settings.jupiter_ang_radius + settings.sun_ang_radius;
    let eclipse = eigc_common::math::smoothstep(
        penumbra,
        penumbra + settings.eclipse_soft,
        std::f32::consts::PI - sep,
    )
    .clamp(0.0, 1.0);

    let phase_brightness = (std::f32::consts::PI - sep) / std::f32::consts::PI;
    let planet_shine =
        (settings.planet_shine_max * phase_brightness).clamp(0.0, settings.planet_shine_max);

    state.sun_dir = sun_dir;
    state.jupiter_dir = jupiter_dir;
    state.eclipse_factor = eclipse;
    state.planet_shine_factor = planet_shine;
}

#[cfg(test)]
mod tests {
    use super::*;
    use eigc_moons::{MoonId, SkyCalibration, TerrainCalibration};

    /// Constrói um `MoonProfile` mínimo para teste, com valores arbitrários.
    fn test_profile() -> MoonProfile {
        MoonProfile {
            moon_id: MoonId::Europa,
            display_name: "TESTEEEEEEEEE LETICIA".to_string(),
            jupiter_angular_diameter_deg: 20.0,
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
                orbital_period_seconds: 100_000.0,
                base_sun_dir: [0.0, 0.0, -1.0],
                base_jupiter_dir: [1.0, 0.0, 0.0],
                jupiter_libration_lat_deg: 4.0,
                jupiter_libration_lon_deg: 8.0,
                jupiter_ang_radius: 0.174_533, // 10° em radianos
                sun_elevation_deg: 30.0,
                eclipse_soft_deg: 2.0,
                planet_shine_max: 0.01,
            },
        }
    }

    /// Testa se campos por lua são transcritos corretamente para `SkySettings`.
    #[test]
    fn from_profile_converts_per_moon_fields_correctly() {
        let profile = test_profile();
        let settings = SkySettings::from_profile(&profile);

        assert_eq!(settings.orbital_period_seconds, 100_000.0);
        assert_eq!(settings.jupiter_ang_radius, 0.174_533);
        assert_eq!(settings.planet_shine_max, 0.01);

        // graus -> radianos
        assert!((settings.jupiter_libration_lat - 4.0_f32.to_radians()).abs() < 1e-6);
        assert!((settings.jupiter_libration_lon - 8.0_f32.to_radians()).abs() < 1e-6);
        assert!((settings.eclipse_soft - 2.0_f32.to_radians()).abs() < 1e-6);
        assert!((settings.sun_elevation - 30.0_f32.to_radians()).abs() < 1e-6);
    }

    /// Testa se direções base chegam normalizadas como `Vec3` em `SkySettings`.
    #[test]
    fn from_profile_converts_and_normalizes_base_directions() {
        let profile = test_profile();
        let settings = SkySettings::from_profile(&profile);

        assert!((settings.base_sun_dir - Vec3::new(0.0, 0.0, -1.0)).length() < 1e-6);
        assert!((settings.base_jupiter_dir - Vec3::new(1.0, 0.0, 0.0)).length() < 1e-6);
        assert!((settings.base_sun_dir.length() - 1.0).abs() < 1e-6);
        assert!((settings.base_jupiter_dir.length() - 1.0).abs() < 1e-6);
    }

    #[test]
    fn from_profile_keeps_universal_fields_independent_of_moon_calibration() {
        let profile = test_profile();
        let settings = SkySettings::from_profile(&profile);

        assert_eq!(settings.sun_illuminance, 4500.0);
        assert_eq!(settings.ambient_brightness, 0.05);
        assert_eq!(settings.orbit_normal, Vec3::Y);

        // derivado de constante fixa (~5.2 UA) e não do perfil da lua
        let expected_sun_ang_radius = (0.53_f32 / 5.2).to_radians() * 0.5;
        assert!((settings.sun_ang_radius - expected_sun_ang_radius).abs() < 1e-6);
    }
}
