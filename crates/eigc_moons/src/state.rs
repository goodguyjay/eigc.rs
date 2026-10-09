//! Define o estado de alto nível da aplicação, controlando a transição entre o menu de seleção,
//! carregar o perfil da lua ativa e ter o terreno pronto para exibição.

use bevy::prelude::{Handle, Resource, States};
use crate::MoonProfile;

/// Estado da aplicação: menu de seleção de lua, carregando o asset de perfil da lua escolhida ou
/// já rodando com terreno e cena montados.
#[derive(States, Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum AppState {
    /// Menu inicial em que o jogador escolhe a lua a explorar.
    #[default]
    MainMenu,
    /// Aguardando o carregamento do `MoonProfile` da lua escolhida no menu.
    LoadingMoonProfile,
    /// Simulação rodando com terreno e céu montados.
    Running,
}

/// Guarda o handle do perfil de lua que a aplicação está aguardando carregar, ou já usando depois de
/// carregado.
#[derive(Resource, Clone)]
pub struct ActiveMoonProfileHandle(pub Handle<MoonProfile>);
