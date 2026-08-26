//! Define os dados de calibração por lua, carregados como asset ron via AssetServer do bevy.

use bevy::prelude::Asset;
use bevy::reflect::TypePath;
use serde::{Deserialize, Serialize};

/// Identifica de forma única cada uma das quatro luas galileanas suportadas.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize, Serialize)]
pub enum MoonId {
    Europa,
    Io,
    Ganymede,
    Callisto,
}

/// Parâmetros de calibração de geração procedural de terreno para uma lua específica.
#[derive(Debug, Clone, Copy, Deserialize)]
pub struct TerrainCalibration {
    /// Semente de geração procedural (para ruído, etc.)
    pub seed: u32,
    /// Frequência base do ruído (menor → características mais amplas)
    pub base_frequency: f32,
    /// Direção normalizada das características lineares (como "lineae" em Europa)
    pub feature_direction: [f32; 2],
    /// Escala vertical do terreno em metros
    pub vertical_amplitude_meters: f32,
    /// Quantidade de deslocamento de distorção (warp) em metros
    pub warp_amplitude_meters: f32,
}

/// Dados completos de uma lua, incluindo parâmetros de calibração de terreno e informações de exibição.
#[derive(Debug, Clone, Asset, TypePath, Deserialize)]
pub struct MoonProfile {
    /// Identificador único da lua.
    pub moon_id: MoonId,
    /// Nome de exibição da lua (para UI, etc.)
    pub display_name: String,
    /// Diâmetro angular da lua em graus
    pub jupiter_angular_diameter_deg: f32,
    /// Parâmetros de geração procedural de terreno específico dessa lua
    pub terrain: TerrainCalibration,
    /// Cor base do material do terreno
    pub terrain_base_color: [f32; 4],
    /// Se esta lua tem modo de caminha completo implementado
    pub walkable: bool,
}
