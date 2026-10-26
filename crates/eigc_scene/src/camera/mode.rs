//! Modo de movimento da câmera: primeira pessoa (padrão) ou voo livre (só em builds de debug).

#[cfg(feature = "dev")]
use bevy::prelude::{ButtonInput, KeyCode, ResMut};
use bevy::prelude::{Res, Resource};

/// Modo de movimento ativo da câmera. A orientação (mouse look e lock) é a mesma nos dois modos.
#[derive(Resource, Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum MovementMode {
    /// Primeira pessoa: anda no plano horizontal e mantém a altura do olho sobre o terreno.
    #[default]
    Walk,
    /// Voo livre, ferramenta de debug: ignora o terreno e voa na direção em que olha.
    FreeFly,
}

#[cfg(feature = "dev")]
impl MovementMode {
    /// Retorna o outro modo.
    fn toggled(self) -> Self {
        match self {
            MovementMode::Walk => MovementMode::FreeFly,
            MovementMode::FreeFly => MovementMode::Walk,
        }
    }
}

/// Condição de sistema: verdadeira enquanto `mode` é o modo de movimento ativo.
pub(super) fn in_mode(mode: MovementMode) -> impl FnMut(Res<MovementMode>) -> bool + Clone {
    move |current| *current == mode
}

/// Alterna entre primeira pessoa e voo livre ao apertar Tab. Só existe com a feature `dev`: em
/// release não há como sair da primeira pessoa.
#[cfg(feature = "dev")]
pub(super) fn toggle_movement_mode(
    keys: Res<ButtonInput<KeyCode>>,
    mut mode: ResMut<MovementMode>,
) {
    if keys.just_pressed(KeyCode::Tab) {
        *mode = mode.toggled();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Testa que o modo padrão é a primeira pessoa.
    #[test]
    fn default_movement_mode_is_walk() {
        assert_eq!(MovementMode::default(), MovementMode::Walk);
    }

    #[cfg(feature = "dev")]
    mod dev {
        use super::*;
        use bevy::prelude::{App, Update};

        /// Testa que `toggled` alterna nos dois sentidos.
        #[test]
        fn toggled_switches_between_both_modes() {
            assert_eq!(MovementMode::Walk.toggled(), MovementMode::FreeFly);
            assert_eq!(MovementMode::FreeFly.toggled(), MovementMode::Walk);
        }

        /// Testa que apertar Tab troca o modo e que sem Tab ele não muda.
        #[test]
        fn tab_toggles_movement_mode() {
            let mut app = App::new();
            app.init_resource::<MovementMode>()
                .init_resource::<ButtonInput<KeyCode>>()
                .add_systems(Update, toggle_movement_mode);

            app.update();
            assert_eq!(
                *app.world().resource::<MovementMode>(),
                MovementMode::Walk,
                "mudou de modo sem Tab"
            );

            app.world_mut()
                .resource_mut::<ButtonInput<KeyCode>>()
                .press(KeyCode::Tab);
            app.update();
            assert_eq!(
                *app.world().resource::<MovementMode>(),
                MovementMode::FreeFly
            );
        }
    }
}
