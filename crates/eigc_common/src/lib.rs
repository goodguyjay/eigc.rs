//! Código e matemática compartilhados entre os crates do workspace.

/// Constantes físicas e de configuração compartilhadas entre crates.
pub mod constants;
/// Configuração de ambiente local do desenvolvedor, fora do controle de versão.
pub mod environment;
/// Ponto de interesse compartilhado para decisões de LOD.
pub mod lod_focus;
/// Funções matemáticas comuns usadas por mais de um crate do workspace.
pub mod math;
