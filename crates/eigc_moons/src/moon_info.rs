//! Dados descritivos estáticos de cada lua galileana, usados para apresentação (menu, fichas).
//!
//! Esta tabela é independente do `MoonProfile` (calibração carregada de `.ron`): os `.ron` de
//! Io, Ganimedes e Calisto ainda não existem, e a apresentação não deve depender deles.

use crate::MoonId;

/// Todas as luas na ordem fixa de exibição: Europa, Io, Ganimedes, Calisto.
pub const MOON_DISPLAY_ORDER: [MoonId; 4] = [
    MoonId::Europa,
    MoonId::Io,
    MoonId::Ganymede,
    MoonId::Callisto,
];

/// Dados de apresentação de uma lua.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MoonInfo {
    /// Lua a que esses dados se referem.
    pub moon_id: MoonId,
    /// Nome exibido na interface, em português.
    pub display_name: &'static str,
    /// Frase curta que resume a lua, exibida abaixo do nome.
    pub tagline: &'static str,
    /// Descrição de duas a três frases sobre a lua e suas características.
    pub summary: &'static str,
    /// Diâmetro médio em quilômetros.
    pub diameter_km: u32,
    /// Gravidade superficial em metros por segundo ao quadrado.
    pub surface_gravity_ms2: f32,
    /// Período orbital ao redor de Júpiter em dias terrestres.
    pub orbital_period_days: f32,
    /// Distância orbital média até Júpiter em quilômetros.
    pub jupiter_distance_km: u32,
    /// Característica dominante da superfície, em poucas palavras.
    pub surface: &'static str,
    /// Indica se a exploração terrestre já está implementada para esta lua.
    ///
    /// Duplica conceitualmente `MoonProfile::walkable` de propósito: o perfil das luas ainda não
    /// implementadas não carrega, então o menu não pode consultá-lo. Unificar quando os `.ron`
    /// existirem (ver `BACKLOG.md`).
    pub available: bool,
}

impl MoonInfo {
    /// Raio médio em quilômetros, derivado do diâmetro.
    pub fn radius_km(&self) -> f32 {
        self.diameter_km as f32 / 2.0
    }
}

const EUROPA: MoonInfo = MoonInfo {
    moon_id: MoonId::Europa,
    display_name: "Europa",
    tagline: "O oceano escondido sob o gelo",
    summary: "Coberta por uma crosta de gelo de água cortada por fraturas longas e lineares, \
              Europa esconde um oceano global de água salgada sob a superfície. É um dos \
              lugares mais promissores do Sistema Solar na busca por ambientes habitáveis.",
    diameter_km: 3_122,
    surface_gravity_ms2: 1.31,
    orbital_period_days: 3.55,
    jupiter_distance_km: 671_100,
    surface: "Gelo de água fraturado",
    available: true,
};

const IO: MoonInfo = MoonInfo {
    moon_id: MoonId::Io,
    display_name: "Io",
    tagline: "O mundo mais vulcânico do Sistema Solar",
    summary: "Esticada e comprimida pela gravidade de Júpiter e das outras luas, Io mantém \
              centenas de vulcões ativos que lançam enxofre e lava a centenas de quilômetros de \
              altura. Sua superfície é constantemente renovada e quase não tem crateras.",
    diameter_km: 3_643,
    surface_gravity_ms2: 1.80,
    orbital_period_days: 1.77,
    jupiter_distance_km: 421_800,
    surface: "Planícies de enxofre e lava",
    available: false,
};

const GANYMEDE: MoonInfo = MoonInfo {
    moon_id: MoonId::Ganymede,
    display_name: "Ganimedes",
    tagline: "A maior lua do Sistema Solar",
    summary: "Maior que o planeta Mercúrio, Ganimedes é a única lua conhecida com campo \
              magnético próprio. Sua superfície mistura terrenos escuros e antigos com regiões \
              claras e sulcadas, sobre um possível oceano subterrâneo.",
    diameter_km: 5_268,
    surface_gravity_ms2: 1.43,
    orbital_period_days: 7.15,
    jupiter_distance_km: 1_070_400,
    surface: "Gelo sulcado e crateras antigas",
    available: false,
};

const CALLISTO: MoonInfo = MoonInfo {
    moon_id: MoonId::Callisto,
    display_name: "Calisto",
    tagline: "A superfície mais antiga e craterizada",
    summary: "Calisto é a mais distante das luas galileanas e uma das superfícies mais \
              craterizadas conhecidas, quase inalterada há bilhões de anos. Fica fora do \
              intenso cinturão de radiação de Júpiter, o que a torna um alvo atraente para \
              futuras bases.",
    diameter_km: 4_821,
    surface_gravity_ms2: 1.24,
    orbital_period_days: 16.69,
    jupiter_distance_km: 1_882_700,
    surface: "Gelo e rocha com crateras",
    available: false,
};

/// Retorna os dados de apresentação da lua informada.
pub fn moon_info(moon_id: MoonId) -> &'static MoonInfo {
    match moon_id {
        MoonId::Europa => &EUROPA,
        MoonId::Io => &IO,
        MoonId::Ganymede => &GANYMEDE,
        MoonId::Callisto => &CALLISTO,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Testa que `moon_info` devolve, para cada lua da ordem de exibição, os
    /// dados cuja `moon_id` é a própria lua.
    #[test]
    fn every_moon_has_matching_info() {
        for moon_id in MOON_DISPLAY_ORDER {
            assert_eq!(moon_info(moon_id).moon_id, moon_id);
        }
    }

    /// Testa que a ordem de exibição não repete nenhuma lua e cobre as quatro
    /// luas galileanas.
    #[test]
    fn display_order_has_no_duplicates_and_covers_all_moons() {
        for (i, a) in MOON_DISPLAY_ORDER.iter().enumerate() {
            for b in &MOON_DISPLAY_ORDER[i + 1..] {
                assert_ne!(a, b);
            }
        }
        assert_eq!(MOON_DISPLAY_ORDER.len(), 4);
    }

    /// Testa que, por enquanto, só Europa está marcada como disponível para
    /// exploração.
    #[test]
    fn only_europa_is_available_for_now() {
        for moon_id in MOON_DISPLAY_ORDER {
            assert_eq!(moon_info(moon_id).available, moon_id == MoonId::Europa);
        }
    }

    /// Testa que o caminho do perfil de Europa aponta para o `europa.ron` que
    /// existe em `assets/moons/`.
    #[test]
    fn profile_asset_path_matches_existing_europa_ron() {
        assert_eq!(MoonId::Europa.profile_asset_path(), "moons/europa.ron");
    }

    /// Testa que o raio é metade do diâmetro, usando Europa (3.122 km) como
    /// referência.
    #[test]
    fn radius_is_half_the_diameter() {
        assert_eq!(moon_info(MoonId::Europa).radius_km(), 1_561.0);
    }
}
