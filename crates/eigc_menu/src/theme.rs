//! Identidade visual do menu: paleta de cores, dimensões e fontes.

use bevy::asset::Handle;
use bevy::prelude::{AssetServer, Color, Resource};
use bevy::text::Font;

/// Texto principal, quase branco com leve tom azulado.
pub(crate) const TEXT_PRIMARY: Color = Color::srgb(0.96, 0.97, 1.0);
/// Texto de apoio, como resumos e subtítulos.
pub(crate) const TEXT_SECONDARY: Color = Color::srgba(0.80, 0.84, 0.92, 0.82);
/// Texto discreto, como rótulos de estatísticas.
pub(crate) const TEXT_MUTED: Color = Color::srgba(0.70, 0.75, 0.85, 0.60);
/// Nome da lua em repouso, na visão geral.
pub(crate) const LABEL_IDLE: Color = Color::srgba(0.88, 0.91, 0.97, 0.78);

/// Cor de destaque, um azul gelo.
pub(crate) const ACCENT: Color = Color::srgb(0.49, 0.77, 1.0);
/// Destaque com o cursor sobre o elemento.
pub(crate) const ACCENT_HOVER: Color = Color::srgb(0.66, 0.85, 1.0);
/// Destaque com o elemento pressionado.
pub(crate) const ACCENT_PRESSED: Color = Color::srgb(0.37, 0.65, 0.92);
/// Texto sobre fundo de destaque.
pub(crate) const ON_ACCENT: Color = Color::srgb(0.02, 0.05, 0.10);

/// Fundo semitransparente do painel de detalhes.
pub(crate) const PANEL_BACKGROUND: Color = Color::srgba(0.045, 0.063, 0.11, 0.80);
/// Borda sutil de painéis e botões secundários.
pub(crate) const BORDER_SUBTLE: Color = Color::srgba(1.0, 1.0, 1.0, 0.10);
/// Borda de botão secundário com o cursor sobre ele.
pub(crate) const BORDER_STRONG: Color = Color::srgba(1.0, 1.0, 1.0, 0.28);
/// Fundo de botão secundário com o cursor sobre ele.
pub(crate) const GHOST_HOVER_BACKGROUND: Color = Color::srgba(1.0, 1.0, 1.0, 0.07);
/// Fundo de botão secundário em repouso.
pub(crate) const GHOST_BACKGROUND: Color = Color::srgba(1.0, 1.0, 1.0, 0.0);
/// Sombra projetada pelo painel.
pub(crate) const PANEL_SHADOW: Color = Color::srgba(0.0, 0.0, 0.0, 0.55);

/// Fundo do selo "Disponível".
pub(crate) const BADGE_AVAILABLE_BACKGROUND: Color = Color::srgba(0.49, 0.77, 1.0, 0.16);
/// Fundo do selo "Em breve".
pub(crate) const BADGE_SOON_BACKGROUND: Color = Color::srgba(1.0, 1.0, 1.0, 0.08);

/// Fundo do aviso de "não implementada".
pub(crate) const TOAST_BACKGROUND: Color = Color::srgba(0.09, 0.07, 0.05, 0.94);
/// Borda e ícone do aviso, em âmbar.
pub(crate) const TOAST_ACCENT: Color = Color::srgb(0.96, 0.74, 0.36);
/// Véu escuro da tela de carregamento.
pub(crate) const LOADING_VEIL: Color = Color::srgba(0.008, 0.012, 0.025, 0.86);

/// Largura do painel de detalhes, em pixels lógicos.
pub(crate) const PANEL_WIDTH: f32 = 440.0;
/// Distância do painel de detalhes à borda direita da janela quando aberto.
pub(crate) const PANEL_MARGIN: f32 = 56.0;
/// Largura da área de texto de cada rótulo de lua na visão geral.
pub(crate) const LABEL_WIDTH: f32 = 220.0;

/// Lado do botão de engrenagem (ele é quadrado, com cantos arredondados em círculo).
pub(crate) const SETTINGS_BUTTON_SIZE: f32 = 42.0;
/// Distância do botão de engrenagem às bordas superior e direita da janela.
pub(crate) const SETTINGS_BUTTON_MARGIN: f32 = 32.0;
/// Largura do painel de configurações.
pub(crate) const SETTINGS_PANEL_WIDTH: f32 = 290.0;
/// Raio dos cantos do painel de configurações.
pub(crate) const SETTINGS_PANEL_RADIUS: f32 = 10.0;
/// Raio dos cantos do container do seletor "Não"/"Sim".
pub(crate) const SETTINGS_SELECTOR_RADIUS: f32 = 7.0;
/// Raio dos cantos de cada opção do seletor "Não"/"Sim", menor que o do container para o destaque
/// não encostar na borda externa.
pub(crate) const SETTINGS_SELECTOR_OPTION_RADIUS: f32 = 5.0;
/// Espaçamento horizontal interno de cada linha do painel de configurações.
pub(crate) const SETTINGS_ROW_PADDING_X: f32 = 16.0;
/// Espaçamento vertical interno de cada linha do painel de configurações.
pub(crate) const SETTINGS_ROW_PADDING_Y: f32 = 14.0;

/// Fundo do botão de engrenagem. Nunca muda, só a borda reage a hover/painel aberto.
pub(crate) const SETTINGS_GEAR_BACKGROUND: Color = Color::srgb_u8(0x10, 0x14, 0x1d);
/// Borda do botão de engrenagem em repouso.
pub(crate) const SETTINGS_GEAR_BORDER: Color = Color::srgb_u8(0x2a, 0x30, 0x3c);
/// Borda do botão de engrenagem com o painel aberto ou o cursor sobre ele.
pub(crate) const SETTINGS_GEAR_BORDER_ACTIVE: Color = Color::srgb_u8(0x6b, 0x73, 0x83);

/// Fundo sólido do painel de configurações (sem transparência, diferente do painel de detalhes).
pub(crate) const SETTINGS_PANEL_BACKGROUND: Color = Color::srgb_u8(0x0d, 0x11, 0x19);
/// Borda do painel de configurações.
pub(crate) const SETTINGS_PANEL_BORDER: Color = Color::srgb_u8(0x26, 0x2c, 0x37);
/// Borda do seletor "Não"/"Sim" e dos cards do painel de configurações.
pub(crate) const SETTINGS_SELECTOR_BORDER: Color = Color::srgb_u8(0x2f, 0x36, 0x43);
/// Fundo dos cards do painel de configurações, um pouco mais claro que o fundo do painel.
pub(crate) const SETTINGS_CARD_BACKGROUND: Color = Color::srgb_u8(0x14, 0x1a, 0x25);
/// Fundo do card "Configurar controles" com o cursor sobre ele.
pub(crate) const SETTINGS_CARD_HOVER_BACKGROUND: Color = Color::srgb_u8(0x1a, 0x21, 0x2d);

/// Texto principal do painel de configurações (título do cabeçalho e dos cards).
pub(crate) const SETTINGS_TEXT_PRIMARY: Color = Color::srgb_u8(0xec, 0xe8, 0xe1);
/// Texto secundário do painel de configurações (descrições, fechar, chevron).
pub(crate) const SETTINGS_TEXT_SECONDARY: Color = Color::srgb_u8(0x8a, 0x90, 0x9c);
/// Cor de destaque dourada/bege: fundo da opção selecionada do seletor "Não"/"Sim".
pub(crate) const SETTINGS_ACCENT: Color = Color::srgb_u8(0xe3, 0xc0, 0x7a);

/// Handles das fontes do menu (Space Grotesk).
#[derive(Resource, Clone)]
pub(crate) struct MenuFonts {
    /// Peso regular, para texto corrido.
    pub regular: Handle<Font>,
    /// Peso médio, para rótulos e valores.
    pub medium: Handle<Font>,
    /// Peso semibold, para nomes.
    pub semibold: Handle<Font>,
    /// Peso bold, para títulos.
    pub bold: Handle<Font>,
}

impl MenuFonts {
    /// Inicia o carregamento das fontes a partir de `assets/fonts/static/`.
    pub(crate) fn load(asset_server: &AssetServer) -> Self {
        Self {
            regular: asset_server.load("fonts/static/SpaceGrotesk-Regular.ttf"),
            medium: asset_server.load("fonts/static/SpaceGrotesk-Medium.ttf"),
            semibold: asset_server.load("fonts/static/SpaceGrotesk-SemiBold.ttf"),
            bold: asset_server.load("fonts/static/SpaceGrotesk-Bold.ttf"),
        }
    }
}
