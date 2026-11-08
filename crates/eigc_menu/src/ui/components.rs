//! Componentes marcadores e estado dos nós de UI do menu.

use bevy::prelude::{Color, Component, Resource};
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

/// Contêiner do painel de configurações, mostrado e escondido pela engrenagem.
#[derive(Component, Debug)]
pub(crate) struct SettingsPanel;

/// Progresso da animação de abertura do painel de configurações (fade-in-down). Vive na própria
/// entidade do painel; `was_visible` é o que permite detectar o instante em que ele volta a ficar
/// visível, para reiniciar a animação do zero a cada abertura.
#[derive(Component, Debug, Default)]
pub(crate) struct SettingsPanelFade {
    /// Segundos decorridos desde que o painel começou a abrir.
    pub elapsed_secs: f32,
    /// Se o painel estava visível no frame anterior.
    pub was_visible: bool,
}

/// Cor de fundo final (antes da animação de abertura) de um elemento do painel de configurações.
/// Guardada à parte para a animação nunca precisar ler `BackgroundColor` de volta, já que esse
/// valor é o que ela mesma altera a cada frame.
#[derive(Component, Debug, Clone, Copy)]
pub(crate) struct SettingsFadeBackground(pub Color);

/// Cor de borda final (antes da animação de abertura) de um elemento do painel de configurações,
/// pelo mesmo motivo de `SettingsFadeBackground`.
#[derive(Component, Debug, Clone, Copy)]
pub(crate) struct SettingsFadeBorder(pub Color);

/// Cor de texto final (antes da animação de abertura) de um elemento do painel de configurações,
/// pelo mesmo motivo de `SettingsFadeBackground`.
#[derive(Component, Debug, Clone, Copy)]
pub(crate) struct SettingsFadeText(pub Color);

/// Cor de tingimento final (antes da animação de abertura) da imagem do painel de configurações,
/// pelo mesmo motivo de `SettingsFadeBackground`.
#[derive(Component, Debug, Clone, Copy)]
pub(crate) struct SettingsFadeImage(pub Color);

/// Botão de engrenagem que abre e fecha o painel de configurações.
#[derive(Component, Debug)]
pub(crate) struct SettingsGearButton;

/// Ícone da engrenagem, que gira quando o painel abre.
#[derive(Component, Debug)]
pub(crate) struct SettingsGearIcon;

/// Linha "Configurar controles" do painel de configurações: clicável, sem ação por enquanto, com
/// fundo que destaca em hover.
#[derive(Component, Debug)]
pub(crate) struct ConfigureControlsRow;

/// Qual opção do seletor "Tela cheia" uma entidade representa (botão ou o texto dentro dele).
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum FullscreenOption {
    /// Tela cheia desligada, valor inicial.
    No,
    /// Tela cheia ligada.
    Yes,
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
