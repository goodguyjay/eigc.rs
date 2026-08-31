//! Define os dados de calibração por lua carregados como asset .ron.

pub mod loader;
pub mod plugin;
pub mod profile;
pub mod state;

pub use loader::{MoonProfileLoader, MoonProfileLoaderError};
pub use plugin::MoonPlugin;
pub use profile::{MoonId, MoonProfile, TerrainCalibration, SkyCalibration};
pub use state::{ActiveMoonProfileHandle, AppState};
