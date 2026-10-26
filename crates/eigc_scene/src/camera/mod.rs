//! Câmera: primeira pessoa (padrão), voo livre de debug e orientação com lock em sol/Júpiter.

/// Câmera em primeira pessoa: caminhada seguindo o terreno.
pub mod first_person;
/// Voo livre, modo de movimento de debug.
pub mod free_fly_camera;
/// Orientação da câmera (mouse look), cursor e `CamLock`/`LockMode`, compartilhados entre modos
/// de movimento.
pub mod look;
/// Modo de movimento ativo (primeira pessoa ou voo livre).
pub mod mode;
/// Plugin que registra a câmera e ordena seus sistemas.
pub mod plugin;
