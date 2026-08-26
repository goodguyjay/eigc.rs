//! Define o estado de alto nível da aplicação, controlando a transição entre carregar o perfil
//! da lua ativa e ter o terreno pronto para exibição.

use bevy::prelude::{Handle, Resource, States};
use crate::MoonProfile;

/// Estado da aplicação, carregando o asset de perfil da lua ou já rodando com terreno e cena
/// montados.
#[derive(States, Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum AppState {
    #[default]
    LoadingMoonProfile,
    Running,
}

/// Guarda o handle do perfil de lua que a aplicação está aguardando carregar, ou já usando depois de
/// carregado.
#[derive(Resource, Clone)]
pub struct ActiveMoonProfileHandle(pub Handle<MoonProfile>);
