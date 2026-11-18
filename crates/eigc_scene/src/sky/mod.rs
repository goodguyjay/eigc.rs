//! Plugin de céu: Júpiter, planet shine e starfield, calibrados por lua ativa.

use bevy::app::App;
use bevy::pbr::MaterialPlugin;
use bevy::prelude::{
    AssetApp, AssetServer, Assets, Commands, GlobalAmbientLight, Handle, Image,
    IntoScheduleConfigs, Message, MessageWriter, OnEnter, OnExit, Plugin, Quat, Res, ResMut,
    Resource, Update, Vec3, error, in_state, resource_exists,
};
use bevy::render::RenderApp;
use eigc_moons::{ActiveMoonProfileHandle, AppState, MoonProfile, SkyGeometry};
use eigc_sim::{SimSet, SimTime};

mod jupiter;
mod planet_shine;
mod shared;
mod starfield;
mod sun;

/// Brilho da luz ambiente, igual em todas as luas. É o único preenchimento das sombras do sol: em
/// Io, Júpiter fica quase na direção do sol e o planet shine ilumina as mesmas faces que ele. Com
/// a exposição padrão e o sol de 4500 lux, 150 deixa a sombra da borda de uma patera em ~27 de
/// 255 na tela contra ~145 do chão iluminado (escura, mas com o piso legível) e clareia o chão
/// iluminado em ~4 níveis. O valor anterior (0,05) deixava toda sombra preta pura.
const AMBIENT_BRIGHTNESS: f32 = 150.0;

/// Piso da luz ambiente à noite, igual em todas as luas (só tem efeito nos céus de lugar real; no
/// modelo `Fixed` o sol nunca se põe). Fisicamente, à noite só chega o planet shine de Júpiter
/// (~11 lux em Ganimedes, ~72 lux em Io), que com a exposição fixa da câmera fica preto na tela.
/// O piso faz o papel do olho adaptado ao escuro: o chão continua legível, bem mais escuro que de
/// dia. Valor calibrado a olho com o app rodando.
const NIGHT_AMBIENT_BRIGHTNESS: f32 = 40.0;

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
    /// Fator de dia, 0.0 = sol abaixo do horizonte, 1.0 = disco do sol inteiro acima dele. No
    /// modelo `Fixed` o sol nunca se põe, e o fator vale sempre 1.0.
    pub daylight_factor: f32,
    /// Rotação do céu (estrelas) desde o instante zero. Identidade no modelo `Fixed`, em que as
    /// estrelas ficam paradas.
    pub sky_rotation: Quat,
}

/// Configurações físicas do céu, calibradas para a lua ativa.
#[derive(Resource, Clone, Default)]
pub struct SkySettings {
    /// Direção base de Júpiter
    pub base_jupiter_dir: Vec3,
    /// Iluminância do sol, em lux
    pub sun_illuminance: f32,
    /// Brilho ambiente de dia, em lux
    pub ambient_brightness: f32,
    /// Brilho ambiente com o sol abaixo do horizonte, em lux (piso da noite)
    pub night_ambient_brightness: f32,
    /// Período orbital, em segundos
    pub orbital_period_seconds: f32,
    /// Eixo em torno do qual o sol gira, no sentido positivo, ao longo da órbita. No modelo
    /// `Fixed` é a vertical; no `Site` é o polo sul celeste, porque o céu aparente gira para oeste.
    pub orbit_normal: Vec3,
    /// Polo norte celeste, no espaço do mundo. Orienta o eixo de rotação de Júpiter no céu.
    pub celestial_north: Vec3,
    /// Inclinação extra do eixo de Júpiter em relação a `celestial_north`, em radianos. No
    /// `Site` vale zero: as luas orbitam no plano do equador de Júpiter.
    pub jupiter_axis_tilt: f32,
    /// Fase da órbita no instante zero da simulação, em radianos (zero = sol oposto a Júpiter).
    pub start_phase: f32,
    /// Se as estrelas giram junto com o céu (`Site`) ou ficam paradas (`Fixed`).
    pub stars_follow_sky: bool,
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
    /// Elevação do sol acima do plano perpendicular a `orbit_normal`, em radianos. No `Fixed` é a
    /// elevação acima do horizonte; no `Site` é a declinação do sol, zero.
    pub sun_elevation: f32,
}

/// Direções de um lugar real na superfície, no espaço do mundo (leste = +X, cima = +Y,
/// norte = -Z).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SiteDirections {
    /// Direção de Júpiter, no ponto sub-joviano do céu (paralaxe ignorada).
    pub jupiter: Vec3,
    /// Polo norte celeste.
    pub celestial_north: Vec3,
}

/// Direções de Júpiter e do polo norte celeste vistas de um lugar na latitude e longitude dadas
/// (graus; longitude medida a partir do ponto sub-joviano, positiva para leste).
///
/// Em coordenadas fixas na lua (X para Júpiter, Z para o polo norte), o lugar fica em
/// `(cos lat cos lon, cos lat sin lon, sin lat)`, com leste `(-sin lon, cos lon, 0)` e norte
/// `(-sin lat cos lon, -sin lat sin lon, cos lat)`.
pub fn site_directions(latitude_deg: f32, longitude_deg: f32) -> SiteDirections {
    let (sin_lat, cos_lat) = latitude_deg.to_radians().sin_cos();
    let (sin_lon, cos_lon) = longitude_deg.to_radians().sin_cos();
    SiteDirections {
        jupiter: Vec3::new(-sin_lon, cos_lat * cos_lon, sin_lat * cos_lon),
        celestial_north: Vec3::new(0.0, sin_lat, -cos_lat),
    }
}

/// Constrói `SkySettings` a partir do `MoonProfile` ativo.
impl SkySettings {
    /// Constrói a calibração de céu a partir de `MoonProfile` ativo.
    pub fn from_profile(profile: &MoonProfile) -> Self {
        let sky = &profile.sky;
        let geometry = match sky.geometry {
            SkyGeometry::Fixed {
                base_jupiter_dir,
                sun_elevation_deg,
            } => GeometrySettings {
                base_jupiter_dir: Vec3::from_array(base_jupiter_dir).normalize(),
                orbit_normal: Vec3::Y,
                celestial_north: Vec3::Y,
                jupiter_axis_tilt: eigc_common::constants::JUPITER_OBLIQUITY_DEG.to_radians(),
                start_phase: 0.0,
                stars_follow_sky: false,
                sun_elevation: sun_elevation_deg.to_radians(),
            },
            SkyGeometry::Site {
                latitude_deg,
                longitude_deg,
                start_orbit_fraction,
            } => {
                let site = site_directions(latitude_deg, longitude_deg);
                GeometrySettings {
                    base_jupiter_dir: site.jupiter,
                    orbit_normal: -site.celestial_north,
                    celestial_north: site.celestial_north,
                    jupiter_axis_tilt: 0.0,
                    start_phase: start_orbit_fraction * std::f32::consts::TAU,
                    stars_follow_sky: true,
                    sun_elevation: 0.0,
                }
            }
        };
        Self {
            base_jupiter_dir: geometry.base_jupiter_dir,
            sun_illuminance: 4500.0, // sol a ~5.2 UA, universal entre as quatro luas
            ambient_brightness: AMBIENT_BRIGHTNESS,
            night_ambient_brightness: NIGHT_AMBIENT_BRIGHTNESS,
            orbital_period_seconds: sky.orbital_period_seconds,
            orbit_normal: geometry.orbit_normal,
            celestial_north: geometry.celestial_north,
            jupiter_axis_tilt: geometry.jupiter_axis_tilt,
            start_phase: geometry.start_phase,
            stars_follow_sky: geometry.stars_follow_sky,
            jupiter_libration_lat: sky.jupiter_libration_lat_deg.to_radians(),
            jupiter_libration_lon: sky.jupiter_libration_lon_deg.to_radians(),
            sun_ang_radius: (0.53_f32 / 5.2).to_radians() * 0.5, // ~0.5° / 5.2 UA universal
            jupiter_ang_radius: sky.jupiter_ang_radius,
            planet_shine_max: sky.planet_shine_max,
            eclipse_soft: sky.eclipse_soft_deg.to_radians(),
            sun_elevation: geometry.sun_elevation,
        }
    }
}

/// Campos de `SkySettings` que dependem da geometria do céu.
struct GeometrySettings {
    base_jupiter_dir: Vec3,
    orbit_normal: Vec3,
    celestial_north: Vec3,
    jupiter_axis_tilt: f32,
    start_phase: f32,
    stars_follow_sky: bool,
    sun_elevation: f32,
}

/// Plugin de céu: Júpiter, planet shine e starfield, calibrados por lua ativa.
impl Plugin for SkyPlugin {
    fn build(&self, app: &mut App) {
        if app.get_sub_app(RenderApp).is_some() {
            app.add_plugins(MaterialPlugin::<jupiter::JupiterMaterial>::default());
        } else {
            app.init_asset::<jupiter::JupiterMaterial>();
        }

        app.add_message::<SkyAssetsLoaded>()
            .add_systems(
                OnEnter(AppState::Running),
                (build_sky_settings, load_sky_assets),
            )
            .add_systems(
                Update,
                check_sky_assets_loaded.run_if(in_state(AppState::Running)),
            )
            .add_systems(OnExit(AppState::Running), clear_sky_session)
            .init_resource::<SkyState>()
            .add_plugins((
                sun::SunPlugin,
                jupiter::JupiterPlugin,
                starfield::StarfieldPlugin,
                planet_shine::PlanetShinePlugin,
            ))
            .add_systems(
                Update,
                animate_sky_physical
                    .in_set(SimSet::Animate)
                    .run_if(resource_exists::<SkySettings>),
            );
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

/// Desfaz o que a entrada em `Running` montou para o céu: calibração, texturas e luz ambiente.
/// As entidades (sol, Júpiter, domo) saem sozinhas por `DespawnOnExit`. Sem isso, entrar de novo
/// numa lua reaproveitaria `SkyAssets` com `all_loaded` já verdadeiro e o céu nunca respawnaria.
fn clear_sky_session(mut commands: Commands) {
    commands.remove_resource::<SkySettings>();
    commands.remove_resource::<SkyAssets>();
    commands.insert_resource(SkyState::default());
    commands.insert_resource(GlobalAmbientLight::default());
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
    *state = sky_state_at(&settings, sim.0);
}

/// Calcula direções do sol e de Júpiter, fator de eclipse e planet shine no instante `t`, em
/// segundos de simulação.
fn sky_state_at(settings: &SkySettings, t: f32) -> SkyState {
    let wob_lat = settings.jupiter_libration_lat
        * (0.3 * t / settings.orbital_period_seconds * 2.0 * std::f32::consts::PI).sin();
    let wob_lon = settings.jupiter_libration_lon
        * (0.2 * t / settings.orbital_period_seconds * 2.0 * std::f32::consts::PI).sin();

    let up = settings.orbit_normal.normalize();
    let right = settings.base_jupiter_dir.normalize().cross(up).normalize();
    let jup_wobble = Quat::from_axis_angle(right, wob_lat) * Quat::from_axis_angle(up, wob_lon);
    let jupiter_dir = (jup_wobble * settings.base_jupiter_dir).normalize();

    let phase =
        (t / settings.orbital_period_seconds) * 2.0 * std::f32::consts::PI + settings.start_phase;
    let rot = Quat::from_axis_angle(up, phase);
    let horizon_dir = (-jupiter_dir).reject_from(up).normalize();
    let elevation = settings.sun_elevation;
    let noon = (horizon_dir * elevation.cos() + up * elevation.sin()).normalize();
    let sun_dir = (rot * noon).normalize();

    let sep = sun_dir.angle_between(-jupiter_dir);
    let eclipse = eclipse_factor(std::f32::consts::PI - sep, settings);

    let phase_brightness = (std::f32::consts::PI - sep) / std::f32::consts::PI;
    let planet_shine =
        (settings.planet_shine_max * phase_brightness).clamp(0.0, settings.planet_shine_max);

    let sun_elevation_above_horizon = sun_dir.y.clamp(-1.0, 1.0).asin();
    let daylight = eigc_common::math::smoothstep(
        -settings.sun_ang_radius,
        settings.sun_ang_radius,
        sun_elevation_above_horizon,
    );

    let sky_rotation = if settings.stars_follow_sky {
        Quat::from_axis_angle(up, phase - settings.start_phase)
    } else {
        Quat::IDENTITY
    };

    SkyState {
        eclipse_factor: eclipse,
        planet_shine_factor: planet_shine,
        sun_dir,
        jupiter_dir,
        daylight_factor: daylight,
        sky_rotation,
    }
}

/// Fator de eclipse dado o ângulo entre o sol e o centro de Júpiter, em radianos.
///
/// A rampa é centrada na borda de Júpiter: vale 0,5 com o centro do sol sobre a borda, 0 com o
/// disco do sol inteiro mais meia suavização para dentro, e 1 com o disco inteiro mais meia
/// suavização para fora. Assim a luz cai enquanto o sol some atrás do disco, e não antes.
fn eclipse_factor(angle_to_jupiter: f32, settings: &SkySettings) -> f32 {
    let half_width = settings.sun_ang_radius + 0.5 * settings.eclipse_soft;
    eigc_common::math::smoothstep(
        settings.jupiter_ang_radius - half_width,
        settings.jupiter_ang_radius + half_width,
        angle_to_jupiter,
    )
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
                warp_amplitude_meters: 20.0,
                perceptual_roughness: 0.5,
                reflectance: 0.3,
            },
            terrain_base_color: [1.0, 1.0, 1.0, 1.0],
            terrain_steep_color: [0.3, 0.2, 0.1, 1.0],
            walkable: true,
            sky: SkyCalibration {
                orbital_period_seconds: 100_000.0,
                geometry: SkyGeometry::Fixed {
                    base_jupiter_dir: [1.0, 0.0, 0.0],
                    sun_elevation_deg: 30.0,
                },
                jupiter_libration_lat_deg: 4.0,
                jupiter_libration_lon_deg: 8.0,
                jupiter_ang_radius: 0.174_533, // 10° em radianos
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

    /// Testa se a direção base de Júpiter chega normalizada como `Vec3` em `SkySettings`.
    #[test]
    fn from_profile_converts_and_normalizes_base_jupiter_direction() {
        let mut profile = test_profile();
        profile.sky.geometry = SkyGeometry::Fixed {
            base_jupiter_dir: [2.0, 0.0, 0.0],
            sun_elevation_deg: 30.0,
        };
        let settings = SkySettings::from_profile(&profile);

        assert!((settings.base_jupiter_dir - Vec3::new(1.0, 0.0, 0.0)).length() < 1e-6);
    }

    /// Calibração com sol e Júpiter na mesma elevação e sem libração: a trajetória do sol passa
    /// pelo centro de Júpiter em meia órbita.
    fn settings_with_central_eclipse() -> SkySettings {
        let mut profile = test_profile();
        profile.sky.geometry = SkyGeometry::Fixed {
            base_jupiter_dir: [1.0, 0.0, 0.0],
            sun_elevation_deg: 0.0,
        };
        profile.sky.jupiter_libration_lat_deg = 0.0;
        profile.sky.jupiter_libration_lon_deg = 0.0;
        SkySettings::from_profile(&profile)
    }

    /// Testa que no início da órbita o sol fica oposto a Júpiter: sem eclipse e com planet shine
    /// máximo (Júpiter "cheio").
    #[test]
    fn sky_state_at_start_puts_sun_opposite_jupiter() {
        let settings = settings_with_central_eclipse();
        let state = sky_state_at(&settings, 0.0);

        assert!((state.sun_dir + state.jupiter_dir).length() < 1e-4);
        assert_eq!(state.eclipse_factor, 1.0);
        assert!((state.planet_shine_factor - settings.planet_shine_max).abs() < 1e-6);
    }

    /// Testa que em meia órbita o sol fica atrás de Júpiter: eclipse total e planet shine nulo
    /// (lado noturno de Júpiter voltado para a lua).
    #[test]
    fn sky_state_at_half_orbit_puts_sun_behind_jupiter() {
        let settings = settings_with_central_eclipse();
        let state = sky_state_at(&settings, settings.orbital_period_seconds * 0.5);

        assert!(state.sun_dir.angle_between(state.jupiter_dir) < 1e-3);
        assert_eq!(state.eclipse_factor, 0.0);
        assert!(state.planet_shine_factor < 1e-4);
    }

    /// Testa que a rampa do eclipse é centrada na borda de Júpiter.
    #[test]
    fn eclipse_factor_is_half_with_sun_center_on_jupiter_limb() {
        let settings = SkySettings::from_profile(&test_profile());

        let at_limb = eclipse_factor(settings.jupiter_ang_radius, &settings);
        assert!((at_limb - 0.5).abs() < 1e-5, "na borda: {at_limb}");
    }

    /// Regressão: a rampa antiga ficava toda do lado de fora do disco, e a luz caía com o sol
    /// ainda inteiro ao lado de Júpiter. Além de meia suavização para fora, a luz é cheia; além
    /// de meia suavização para dentro, o eclipse é total.
    #[test]
    fn eclipse_factor_is_full_outside_and_zero_inside_the_ramp() {
        let settings = SkySettings::from_profile(&test_profile());
        let half_width = settings.sun_ang_radius + 0.5 * settings.eclipse_soft;

        let outside = eclipse_factor(settings.jupiter_ang_radius + half_width + 1e-4, &settings);
        let inside = eclipse_factor(settings.jupiter_ang_radius - half_width - 1e-4, &settings);

        assert_eq!(outside, 1.0);
        assert_eq!(inside, 0.0);
    }

    /// Testa se campos universais (sol, brilho ambiente) permanecem constantes independentemente
    /// da calibração da lua, e se o modelo `Fixed` gira o sol em torno da vertical.
    #[test]
    fn from_profile_keeps_universal_fields_independent_of_moon_calibration() {
        let profile = test_profile();
        let settings = SkySettings::from_profile(&profile);

        assert_eq!(settings.sun_illuminance, 4500.0);
        assert_eq!(settings.ambient_brightness, AMBIENT_BRIGHTNESS);
        assert_eq!(settings.night_ambient_brightness, NIGHT_AMBIENT_BRIGHTNESS);
        assert!(settings.night_ambient_brightness < settings.ambient_brightness);
        assert_eq!(settings.orbit_normal, Vec3::Y);

        // derivado de constante fixa (~5.2 UA) e não do perfil da lua
        let expected_sun_ang_radius = (0.53_f32 / 5.2).to_radians() * 0.5;
        assert!((settings.sun_ang_radius - expected_sun_ang_radius).abs() < 1e-6);
    }

    /// Perfil real de Europa, o único no modelo `Fixed`.
    fn europa_profile() -> MoonProfile {
        ron::de::from_str(include_str!("../../../../assets/moons/europa.ron"))
            .expect("o europa.ron deveria ser um MoonProfile válido")
    }

    /// Regressão: o céu de Europa não pode mudar com a entrada do modelo `Site`. Os valores foram
    /// tirados de `sky_state_at` antes da mudança, com o `europa.ron` de então: (fração da órbita,
    /// direção do sol, direção de Júpiter, eclipse, planet shine).
    #[test]
    fn europa_sky_keeps_the_values_from_before_the_site_model() {
        const SNAPSHOT: [(f32, [f32; 3], [f32; 3], f32, f32); 7] = [
            (
                0.0,
                [-0.328_487_84, 0.422_618_27, 0.844_683_05],
                [0.350_878_3, 0.250_627_37, -0.902_258_5],
                1.0,
                0.004_682_845_4,
            ),
            (
                0.13,
                [0.395_528_14, 0.422_618_2, 0.815_445_4],
                [0.344_968_62, 0.258_816_18, -0.902_225_55],
                1.0,
                0.003_977_779_8,
            ),
            (
                0.37,
                [0.835_174_3, 0.422_618_27, -0.351_962_54],
                [0.334_589_66, 0.272_263_23, -0.902_176_56],
                1.0,
                0.001_486_638,
            ),
            (
                0.5,
                [0.310_953_35, 0.422_618_33, -0.851_294_3],
                [0.329_588_77, 0.277_856_74, -0.902_312_04],
                1.0,
                0.000_295_588_25,
            ),
            (
                0.71,
                [-0.750_664_95, 0.422_618_27, -0.507_834_6],
                [0.322_946_97, 0.283_347_76, -0.903_005_7],
                1.0,
                0.002_345_756_4,
            ),
            (
                0.93,
                [-0.636_281_6, 0.422_618_33, 0.645_398_7],
                [0.318_395_38, 0.283_689_08, -0.904_513_6],
                1.0,
                0.004_393_17,
            ),
            (
                2.41,
                [0.727_850_26, 0.422_618_27, -0.540_025_7],
                [0.350_238_3, 0.217_178_51, -0.911_134_9],
                1.0,
                0.001_099_753_8,
            ),
        ];
        let settings = SkySettings::from_profile(&europa_profile());

        for (fraction, sun, jupiter, eclipse, planet_shine) in SNAPSHOT {
            let state = sky_state_at(&settings, fraction * settings.orbital_period_seconds);
            assert!((state.sun_dir - Vec3::from_array(sun)).length() < 1e-5, "sol em {fraction}");
            assert!(
                (state.jupiter_dir - Vec3::from_array(jupiter)).length() < 1e-5,
                "Júpiter em {fraction}"
            );
            assert!((state.eclipse_factor - eclipse).abs() < 1e-5, "eclipse em {fraction}");
            assert!(
                (state.planet_shine_factor - planet_shine).abs() < 1e-6,
                "planet shine em {fraction}"
            );
            assert_eq!(state.daylight_factor, 1.0, "Europa não tem noite: {fraction}");
            assert_eq!(state.sky_rotation, Quat::IDENTITY, "estrelas paradas: {fraction}");
        }
    }

    /// Testa que Júpiter fica na elevação asin(cos lat cos lon), a oeste para longitude
    /// positiva, e que o polo norte celeste fica na elevação da latitude, ao norte (-Z).
    #[test]
    fn site_directions_put_jupiter_and_the_pole_where_the_latitude_says() {
        let site = site_directions(40.0, 58.0);
        let expected_jupiter_elevation =
            (40.0_f32.to_radians().cos() * 58.0_f32.to_radians().cos()).asin();

        assert!((site.jupiter.length() - 1.0).abs() < 1e-6);
        assert!((site.jupiter.y.asin() - expected_jupiter_elevation).abs() < 1e-5);
        assert!(site.jupiter.x < 0.0, "Júpiter deveria ficar a oeste: {:?}", site.jupiter);
        assert!((site.celestial_north.y.asin() - 40.0_f32.to_radians()).abs() < 1e-5);
        assert!(site.celestial_north.z < 0.0, "o polo deveria ficar ao norte");
        assert!(site.jupiter.dot(site.celestial_north).abs() < 1e-6, "Júpiter fora do equador");
    }

    /// Calibração `Site` sem libração, começando na fração `start_orbit_fraction` da órbita.
    fn site_settings(
        latitude_deg: f32,
        longitude_deg: f32,
        start_orbit_fraction: f32,
    ) -> SkySettings {
        let mut profile = test_profile();
        profile.sky.geometry = SkyGeometry::Site {
            latitude_deg,
            longitude_deg,
            start_orbit_fraction,
        };
        profile.sky.jupiter_libration_lat_deg = 0.0;
        profile.sky.jupiter_libration_lon_deg = 0.0;
        SkySettings::from_profile(&profile)
    }

    /// Estado do céu em `fraction` da órbita.
    fn state_at_fraction(settings: &SkySettings, fraction: f32) -> SkyState {
        sky_state_at(settings, fraction * settings.orbital_period_seconds)
    }

    /// Elevação do sol acima do horizonte, em graus, em `fraction` da órbita.
    fn sun_elevation_deg_at(settings: &SkySettings, fraction: f32) -> f32 {
        state_at_fraction(settings, fraction).sun_dir.y.asin().to_degrees()
    }

    /// Testa que, num lugar real, o sol sobe até 90 - latitude e desce até -(90 - latitude):
    /// metade da órbita é noite.
    #[test]
    fn site_sun_peaks_at_ninety_minus_latitude_and_sets_for_half_the_orbit() {
        let settings = site_settings(40.0, 58.0, 0.0);
        let elevations: Vec<f32> = (0..720)
            .map(|step| sun_elevation_deg_at(&settings, step as f32 / 720.0))
            .collect();
        let highest = elevations.iter().copied().fold(f32::NEG_INFINITY, f32::max);
        let lowest = elevations.iter().copied().fold(f32::INFINITY, f32::min);
        let night = elevations.iter().filter(|elevation| **elevation < 0.0).count();

        assert!((highest - 50.0).abs() < 0.5, "máximo {highest}");
        assert!((lowest + 50.0).abs() < 0.5, "mínimo {lowest}");
        assert!((night as f32 / 720.0 - 0.5).abs() < 0.01, "noite em {night} de 720");
    }

    /// Testa que o sol nasce a leste (+X), e que o fator de dia vale 1 ao meio-dia e 0 à
    /// meia-noite.
    #[test]
    fn site_sun_rises_in_the_east_and_daylight_follows_it() {
        let settings = site_settings(40.0, 58.0, 0.0);
        let fractions: Vec<f32> = (0..720).map(|step| step as f32 / 720.0).collect();

        let sunrise = fractions
            .windows(2)
            .find(|pair| {
                sun_elevation_deg_at(&settings, pair[0]) < 0.0
                    && sun_elevation_deg_at(&settings, pair[1]) >= 0.0
            })
            .expect("o sol deveria nascer uma vez por órbita");
        let at_sunrise = state_at_fraction(&settings, sunrise[1]).sun_dir;
        assert!(at_sunrise.x > 0.99, "nasceu fora do leste: {at_sunrise:?}");

        let noon = fractions
            .iter()
            .copied()
            .max_by(|a, b| {
                sun_elevation_deg_at(&settings, *a).total_cmp(&sun_elevation_deg_at(&settings, *b))
            })
            .expect("órbita não vazia");
        let midnight = (noon + 0.5).fract();
        assert_eq!(state_at_fraction(&settings, noon).daylight_factor, 1.0);
        assert_eq!(state_at_fraction(&settings, midnight).daylight_factor, 0.0);
    }

    /// Testa que, num lugar real, o sol passa pelo centro de Júpiter em meia órbita (eclipse
    /// total) e que as estrelas giram com o céu.
    #[test]
    fn site_sun_crosses_jupiter_at_half_orbit_and_the_stars_turn() {
        let settings = site_settings(20.0, 64.0, 0.0);
        let state = state_at_fraction(&settings, 0.5);

        assert!(state.sun_dir.angle_between(state.jupiter_dir) < 1e-3);
        assert_eq!(state.eclipse_factor, 0.0);
        let turned = state.sky_rotation * Vec3::X;
        assert!(turned.angle_between(Vec3::X) > 0.1, "estrelas paradas: {turned:?}");
    }

    /// Testa que `start_orbit_fraction` adianta o céu: começar em 0,5 é começar no eclipse, com
    /// as estrelas na posição inicial.
    #[test]
    fn site_start_orbit_fraction_shifts_the_sky() {
        let state = state_at_fraction(&site_settings(20.0, 64.0, 0.5), 0.0);

        assert_eq!(state.eclipse_factor, 0.0);
        assert_eq!(state.sky_rotation, Quat::IDENTITY);
    }
}
