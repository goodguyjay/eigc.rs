//! Código e matemática compartilhados entre os crates do workspace.

/// Constantes físicas e de configuração compartilhadas entre crates.
pub mod constants;
/// Ponto de interesse compartilhado para decisões de LOD.
pub mod lod_focus;
/// Funções matemáticas comuns usadas por mais de um crate do workspace.
pub mod math;
