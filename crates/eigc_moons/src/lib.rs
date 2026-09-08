//! Define os dados de calibração por lua carregados como asset .ron.

/// Carregador de asset `.ron` para `MoonProfile`.
pub mod loader;
/// Plugin Bevy que registra o loader e os recursos de perfil de lua.
pub mod plugin;
/// Estruturas de dados que descrevem o perfil físico e visual de uma lua.
pub mod profile;
/// Estado de alto nível da aplicação e handle do perfil de lua ativo.
pub mod state;

pub use loader::{MoonProfileLoader, MoonProfileLoaderError};
pub use plugin::MoonPlugin;
pub use profile::{MoonId, MoonProfile, TerrainCalibration, SkyCalibration};
pub use state::{ActiveMoonProfileHandle, AppState};
