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
    /// Quantidade de distorção (warp) em frequência
    pub perceptual_roughness: f32,
    /// O quanto o material do terreno reflete a luz
    pub reflectance: f32,
}

/// Parâmetros de calibração do céu específicos por lua e ajustes artísticos de eclipse/planetshine.
#[derive(Debug, Clone, Deserialize)]
pub struct SkyCalibration {
    /// Período orbital da lua ao redor de Júpiter em segundos
    pub orbital_period_seconds: f32,
    /// Direção base de Júpiter
    pub base_sun_dir: [f32; 3],
    /// Direção base de Júpiter
    pub base_jupiter_dir: [f32; 3],
    /// Amplitude artística de libração de Júpiter em graus
    pub jupiter_libration_lat_deg: f32,
    /// Amplitude artística de libração de Júpiter em graus
    pub jupiter_libration_lon_deg: f32,
    /// Raio angular de Júpiter visto da lua em radianos
    pub jupiter_ang_radius: f32,
    /// Elevação do sol acima do horizonte ao meio-dia local em graus
    pub sun_elevation_deg: f32,
    /// Largura da transição suave de entrada/saída de eclipse em graus
    pub eclipse_soft_deg: f32,
    /// Intensidade máxima da luz refletida de Júpiter em lux
    pub planet_shine_max: f32,
}

/// Dados completos de uma lua, incluindo parâmetros de calibração de terreno e informações de exibição.
#[derive(Debug, Clone, Asset, TypePath, Deserialize)]
pub struct MoonProfile {
    /// Identificador único da lua.
    pub moon_id: MoonId,
    /// Nome de exibição da lua (para UI, etc.)
    pub display_name: String,
    /// Diâmetro angular de Júpiter visto da superfície dessa lua, em graus.
    pub jupiter_angular_diameter_deg: f32,
    /// Parâmetros de geração procedural de terreno específico dessa lua
    pub terrain: TerrainCalibration,
    /// Cor base do material do terreno
    pub terrain_base_color: [f32; 4],
    /// Se esta lua tem modo de caminha completo implementado
    pub walkable: bool,
    /// Parâmetros de calibração do céu e ajustes artísticos
    pub sky: SkyCalibration,
}
