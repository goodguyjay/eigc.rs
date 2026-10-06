//! Interface do menu: construção dos nós de UI e sistemas que os atualizam.

mod components;
mod spawn;
mod update;

pub(crate) use components::ToastState;
pub(crate) use spawn::{spawn_loading_overlay, spawn_menu_ui};
pub(crate) use update::{
    highlight_labels, position_moon_labels, refresh_panel_content, slide_detail_panel,
    style_buttons, update_overview_visibility, update_toast,
};
