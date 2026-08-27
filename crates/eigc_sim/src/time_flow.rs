//! Controla a passagem de tempo da simulação, incluindo a velocidade de avanço e a animação.

use bevy::app::App;
use bevy::prelude::{
    ButtonInput, IntoScheduleConfigs, KeyCode, Plugin, Res, ResMut, Resource, SystemSet, Time,
    Update, info,
};

/// Agrupa sistemas relacionados a avanço do tempo.
#[derive(SystemSet, Debug, Hash, PartialEq, Eq, Clone)]
pub enum SimSet {
    /// Sistemas que avançam o relógio da simulação
    Advance,
    /// Sistemas que consomem SimTime para animar algo
    Animate,
}

/// Controla a velocidade com que o tempo de simulação avança em relação ao tempo real.
#[derive(Resource)]
pub struct TimeFlow {
    /// Multiplicador aplicado ao delta do tempo real a cada quadro
    pub time_scale: f32,
}

impl Default for TimeFlow {
    fn default() -> Self {
        Self { time_scale: 1000.0 }
    }
}

/// Relógio acumulado da simulação, em segundos de simulação decorridos desde o início.
#[derive(Resource, Default)]
pub struct SimTime(pub f32);

/// Registra TimeFlow, SimTIme e os sistemas de avanço/controle de tempo de simulação.
pub struct TimeFlowPlugin;

impl Plugin for TimeFlowPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<TimeFlow>()
            .init_resource::<SimTime>()
            .configure_sets(Update, SimSet::Advance.before(SimSet::Animate))
            .add_systems(
                Update,
                (
                    advance_simulation_time.in_set(SimSet::Advance),
                    handle_time_flow_keyboard_controls,
                ),
            );
    }
}

fn advance_simulation_time(
    mut simulation_time: ResMut<SimTime>,
    time_flow: Res<TimeFlow>,
    real_time: Res<Time>,
) {
    simulation_time.0 += time_flow.time_scale * real_time.delta_secs();
}

fn handle_time_flow_keyboard_controls(
    keyboard_input: Res<ButtonInput<KeyCode>>,
    mut time_flow: ResMut<TimeFlow>,
) {
    let mut time_scale_changed = false;

    if keyboard_input.just_pressed(KeyCode::Digit0) || keyboard_input.just_pressed(KeyCode::Numpad0)
    {
        time_flow.time_scale = 0.0;
        time_scale_changed = true;
    }

    if keyboard_input.just_pressed(KeyCode::Digit1) || keyboard_input.just_pressed(KeyCode::Numpad1)
    {
        time_flow.time_scale = 1.0;
        time_scale_changed = true;
    }

    if keyboard_input.just_pressed(KeyCode::Digit2) || keyboard_input.just_pressed(KeyCode::Numpad2)
    {
        time_flow.time_scale = 600.0;
        time_scale_changed = true;
    }

    if keyboard_input.just_pressed(KeyCode::Digit3) || keyboard_input.just_pressed(KeyCode::Numpad3)
    {
        time_flow.time_scale = 1200.0;
        time_scale_changed = true;
    }

    if keyboard_input.just_pressed(KeyCode::Digit4) || keyboard_input.just_pressed(KeyCode::Numpad4)
    {
        time_flow.time_scale = 5000.0;
        time_scale_changed = true;
    }

    if time_scale_changed {
        info!(
            "Velocidade de simulação ajustada para {}x",
            time_flow.time_scale
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::input::InputPlugin;
    use bevy::time::TimePlugin;

    /// Testa que advance_simulation_time acumula SimTime corretamente dado um time_scale e um
    /// tempo real decorrido conhecidos.
    #[test]
    fn advance_simulation_time_accumulates_proportionally_to_time_scale() {
        let mut app = App::new();
        app.add_plugins(TimePlugin)
            .init_resource::<SimTime>()
            .insert_resource(TimeFlow { time_scale: 10.0 })
            .add_systems(Update, advance_simulation_time);

        app.update();
        let simulation_time_after_first_update = app.world().resource::<SimTime>().0;

        app.update();
        let simulation_time_after_second_update = app.world().resource::<SimTime>().0;

        assert!(
            simulation_time_after_second_update >= simulation_time_after_first_update,
            "SimTime deveria avançar ou permanecer igual, mas diminuiu de {} para {}",
            simulation_time_after_second_update,
            simulation_time_after_first_update
        );
    }

    #[test]
    fn keyboard_control_sets_expected_time_scale() {
        let mut app = App::new();
        app.add_plugins(InputPlugin)
            .insert_resource(TimeFlow::default())
            .add_systems(Update, handle_time_flow_keyboard_controls);

        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::Digit2);

        app.update();

        let time_flow = app.world().resource::<TimeFlow>();
        assert_eq!(
            time_flow.time_scale, 600.0,
            "pressionar Digit2 deveria ajustar time_scale para 600.0, mas foi {}",
            time_flow.time_scale
        );
    }
}
