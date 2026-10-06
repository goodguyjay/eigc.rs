//! Sistema responsável por transicionar a aplicação para o estado running assim que o perfil da
//! lua escolhida no menu termina de carregar. O carregamento em si é disparado pelo `eigc_menu`.

use bevy::prelude::{Assets, NextState, Res, ResMut, State};
use eigc_moons::profile::MoonProfile;
use eigc_moons::state::{ActiveMoonProfileHandle, AppState};

/// Transiciona para `Running` assim que o perfil da lua ativa estiver carregado.
///
/// Deve ser registrado com `run_if(resource_exists::<ActiveMoonProfileHandle>)`, pois o handle só
/// existe depois que o menu escolhe uma lua.
pub fn transition_when_moon_profile_loaded(
    active_handle: Res<ActiveMoonProfileHandle>,
    moon_profiles: Res<Assets<MoonProfile>>,
    current_state: Res<State<AppState>>,
    mut next_state: ResMut<NextState<AppState>>,
) {
    // Se o estado atual já é Running, não faz nada. Essa verificação vem de um bugfix insuportável
    // de chamar esse sistema mesmo quando o estado já é Running, o que fazia a aplicação travar horrivelmente.
    if *current_state.get() == AppState::Running {
        return;
    }
    if moon_profiles.get(&active_handle.0).is_some() {
        next_state.set(AppState::Running);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::prelude::*;
    use bevy::state::app::StatesPlugin;
    use eigc_moons::{MoonId, SkyCalibration, TerrainCalibration};

    /// Conta quantas vezes `OnEnter(AppState::Running)` disparou.
    /// Esse teste vem da correção de um bug em que o sistema `transition_when_moon_profile_loaded`
    /// era chamado mesmo quando o estado já era `Running`, deixando a aplicação rodar sem nem um
    /// mísero fps. (╯°□°)╯︵ ┻━┻
    #[derive(Resource, Default)]
    struct EnterRunningCount(u32);

    fn count_enter_running(mut count: ResMut<EnterRunningCount>) {
        count.0 += 1;
    }

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
            terrain_valley_color: [0.0, 0.0, 0.0, 1.0],
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

    #[test]
    fn moon_profile_loaded_transitions_to_running_only_once() {
        let mut app = App::new();

        let mut profiles = Assets::<MoonProfile>::default();
        let handle = profiles.add(test_profile());

        app.add_plugins(StatesPlugin)
            .init_state::<AppState>()
            .insert_resource(profiles)
            .insert_resource(ActiveMoonProfileHandle(handle))
            .init_resource::<EnterRunningCount>()
            .add_systems(OnEnter(AppState::Running), count_enter_running)
            .add_systems(Update, transition_when_moon_profile_loaded);

        for _ in 0..10 {
            app.update();
        }

        let count = app.world().resource::<EnterRunningCount>().0;
        assert_eq!(
            count, 1,
            "OnEnter deveria disparar exatamente uma vez, mas disparou {} vezes",
            count
        );

        let state = app.world().resource::<State<AppState>>();
        assert_eq!(*state.get(), AppState::Running);
    }
}
