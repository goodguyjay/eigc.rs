//! Ferramental de debug/performance do projeto, disponível apenas na feature `dev`.
//! Nunca deve ser incluído em build de release.
#![cfg(feature = "dev")]

/// Plugin e sistemas de overlay de debug (fps, diagnostics).
pub mod debug_tools;

