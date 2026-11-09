//! Menu inicial da aplicação: mostra as quatro luas galileanas em 3D e inicia a exploração da
//! lua escolhida. Também tem o menu de pausa (ESC) da exploração, que volta para cá.

mod explore;
mod interaction;
mod layout;
mod moon_stats;
mod pause;
mod plugin;
mod scene;
mod settings;
mod theme;
mod ui;

pub use explore::ExploreRequested;
pub use plugin::{MenuPlugin, ShowMenu};
