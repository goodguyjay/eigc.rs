//! Ponto de saída controlado da aplicação: reage ao `AppExit` antes do Bevy encerrar o processo.
//!
//! Hoje não há estado a persistir (sem escrita em disco nem preferências), então a saída só loga
//! o motivo e devolve o cursor ao sistema. Quando surgir persistência, o flush entra em
//! [`handle_app_exit`].

use bevy::app::Last;
use bevy::prelude::{App, AppExit, MessageReader, Plugin, Query, info, warn};
use bevy::window::{CursorGrabMode, CursorOptions};

/// Registra o tratamento de `AppExit` em `Last`, depois de todos os sistemas do quadro.
pub struct ShutdownPlugin;

impl Plugin for ShutdownPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Last, handle_app_exit);
    }
}

/// Código de saída do processo: `0` para `Success`, o código informado para `Error`.
fn exit_code(exit: &AppExit) -> u8 {
    match exit {
        AppExit::Success => 0,
        AppExit::Error(code) => code.get(),
    }
}

/// Loga o motivo da saída e solta o cursor, para que ele não fique preso em saída por erro.
fn handle_app_exit(mut exits: MessageReader<AppExit>, mut cursors: Query<&mut CursorOptions>) {
    let Some(exit) = exits.read().next() else {
        return;
    };

    match exit {
        AppExit::Success => info!("Encerrando a aplicação (código 0)."),
        AppExit::Error(_) => warn!(
            "Encerrando a aplicação com erro (código {}).",
            exit_code(exit)
        ),
    }

    for mut cursor in &mut cursors {
        cursor.grab_mode = CursorGrabMode::None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::MinimalPlugins;
    use bevy::prelude::*;
    use std::num::NonZero;

    fn shutdown_app() -> App {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins).add_plugins(ShutdownPlugin);
        app
    }

    fn locked_cursor(app: &mut App) -> Entity {
        app.world_mut()
            .spawn(CursorOptions {
                grab_mode: CursorGrabMode::Locked,
                ..default()
            })
            .id()
    }

    /// Testa que `Success` vira código 0 e `Error` preserva o código informado.
    #[test]
    fn exit_code_maps_success_and_error() {
        assert_eq!(exit_code(&AppExit::Success), 0);
        assert_eq!(exit_code(&AppExit::Error(NonZero::new(7).unwrap())), 7);
    }

    /// Testa que um `AppExit` solta o cursor que estava travado pela câmera free-fly.
    #[test]
    fn app_exit_releases_locked_cursor() {
        let mut app = shutdown_app();
        let window = locked_cursor(&mut app);

        app.world_mut().write_message(AppExit::Success);
        app.update();

        let cursor = app.world().get::<CursorOptions>(window).unwrap();
        assert_eq!(cursor.grab_mode, CursorGrabMode::None);
    }

    /// Testa que sem `AppExit` o cursor travado continua travado.
    #[test]
    fn cursor_stays_locked_without_app_exit() {
        let mut app = shutdown_app();
        let window = locked_cursor(&mut app);

        app.update();

        let cursor = app.world().get::<CursorOptions>(window).unwrap();
        assert_eq!(cursor.grab_mode, CursorGrabMode::Locked);
    }
}
