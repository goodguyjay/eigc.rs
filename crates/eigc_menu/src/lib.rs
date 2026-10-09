//! Menu inicial da aplicação: mostra as quatro luas galileanas em 3D e inicia a exploração da
//! lua escolhida.

mod explore;
mod interaction;
mod layout;
mod moon_stats;
mod plugin;
mod scene;
mod theme;
mod ui;

pub use explore::ExploreRequested;
pub use plugin::MenuPlugin;
