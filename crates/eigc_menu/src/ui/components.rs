//! Componentes marcadores e estado dos nós de UI do menu.

use bevy::prelude::{Component, Resource};
use eigc_moons::MoonId;

/// Marca os nós visíveis só na visão geral (título e subtítulo).
#[derive(Component, Debug)]
pub(crate) struct OverviewUi;

/// Raiz do rótulo de uma lua na visão geral, posicionado sob a lua em coordenadas de tela.
#[derive(Component, Debug)]
pub(crate) struct MoonLabel(pub MoonId);

/// Texto com o nome da lua dentro do seu rótulo, destacado em hover e foco.
#[derive(Component, Debug)]
pub(crate) struct MoonLabelName(pub MoonId);

/// Contêiner do painel de detalhes, que desliza para dentro e para fora da tela.
#[derive(Component, Debug)]
pub(crate) struct DetailPanel;

/// Campo de texto do painel de detalhes, preenchido a partir da lua em foco.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PanelField {
    /// Nome da lua.
    Name,
    /// Frase curta abaixo do nome.
    Tagline,
    /// Descrição da lua.
    Summary,
    /// Valor da estatística de índice dado, na ordem de `STAT_LABELS`.
    Stat(usize),
}

/// Tipo visual de um botão, que define as cores em repouso, hover e pressionado.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ButtonKind {
    /// Ação principal, com fundo de destaque.
    Primary,
    /// Ação secundária, só com borda.
    Ghost,
}

/// Raiz do aviso de "não implementada".
#[derive(Component, Debug)]
pub(crate) struct ToastRoot;

/// Texto do aviso de "não implementada".
#[derive(Component, Debug)]
pub(crate) struct ToastText;

/// Tempo restante, em segundos, até o aviso sumir. Zero significa oculto.
#[derive(Resource, Default, Debug)]
pub(crate) struct ToastState {
    /// Segundos restantes de exibição.
    pub remaining_secs: f32,
}
