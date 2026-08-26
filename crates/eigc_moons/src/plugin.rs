//! Plugin do Bevy para o MoonProfile e seu carregador de asset correspondente.

use crate::profile::MoonProfile;
use crate::loader::MoonProfileLoader;
use bevy::prelude::{App, AssetApp, Plugin};

//// Plugin do Bevy que registra o MoonProfile e seu carregador de asset correspondente.
pub struct MoonPlugin;

/// Implementação do Plugin para o MoonPlugin, registrando o asset e seu loader no Bevy.
impl Plugin for MoonPlugin {
    fn build(&self, app: &mut App) {
        app.init_asset::<MoonProfile>()
            .init_asset_loader::<MoonProfileLoader>();
    }
}
