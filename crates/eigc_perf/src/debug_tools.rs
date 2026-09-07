#![cfg(feature = "dev")]

//! ferramentas de debug ativas apenas em builds com a feature `dev`.

use bevy::app::{App, Plugin};
use bevy::dev_tools::fps_overlay::FpsOverlayPlugin;
use bevy::diagnostic::{
    EntityCountDiagnosticsPlugin, LogDiagnosticsPlugin,
};

/// Plugin que agrupa overlay de fps e diagnósticos de log apenas quando a feature `dev` está ativa.
pub struct DebugToolsPlugin;

impl Plugin for DebugToolsPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((
            FpsOverlayPlugin::default(),
            LogDiagnosticsPlugin::default(),
            EntityCountDiagnosticsPlugin::default(),
        ));
    }
}
