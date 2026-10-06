//! Formatação, em português, das estatísticas de uma lua exibidas na ficha do menu.

use eigc_moons::MoonInfo;

/// Rótulos das estatísticas, na ordem em que aparecem na ficha.
pub(crate) const STAT_LABELS: [&str; 5] = [
    "Diâmetro",
    "Gravidade",
    "Período orbital",
    "Distância de Júpiter",
    "Superfície",
];

/// Formata um inteiro com ponto como separador de milhar (`671100` vira `671.100`).
fn format_thousands(value: u32) -> String {
    let digits = value.to_string();
    let mut grouped = String::with_capacity(digits.len() + digits.len() / 3);
    for (index, digit) in digits.chars().enumerate() {
        if index > 0 && (digits.len() - index) % 3 == 0 {
            grouped.push('.');
        }
        grouped.push(digit);
    }
    grouped
}

/// Formata um decimal com vírgula como separador e a quantidade de casas indicada.
fn format_decimal(value: f32, decimals: usize) -> String {
    format!("{value:.decimals$}").replace('.', ",")
}

/// Valores das estatísticas de uma lua, na mesma ordem de `STAT_LABELS`.
pub(crate) fn stat_values(info: &MoonInfo) -> [String; 5] {
    [
        format!("{} km", format_thousands(info.diameter_km)),
        format!("{} m/s²", format_decimal(info.surface_gravity_ms2, 2)),
        format!("{} dias", format_decimal(info.orbital_period_days, 2)),
        format!("{} km", format_thousands(info.jupiter_distance_km)),
        info.surface.to_string(),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use eigc_moons::{MoonId, moon_info};

    #[test]
    fn thousands_are_separated_by_dots() {
        assert_eq!(format_thousands(0), "0");
        assert_eq!(format_thousands(999), "999");
        assert_eq!(format_thousands(1_000), "1.000");
        assert_eq!(format_thousands(3_122), "3.122");
        assert_eq!(format_thousands(671_100), "671.100");
        assert_eq!(format_thousands(1_882_700), "1.882.700");
    }

    #[test]
    fn decimals_use_a_comma() {
        assert_eq!(format_decimal(1.31, 2), "1,31");
        assert_eq!(format_decimal(16.69, 2), "16,69");
        assert_eq!(format_decimal(2.0, 2), "2,00");
    }

    #[test]
    fn europa_stats_are_formatted_in_portuguese() {
        let values = stat_values(moon_info(MoonId::Europa));
        assert_eq!(values[0], "3.122 km");
        assert_eq!(values[1], "1,31 m/s²");
        assert_eq!(values[2], "3,55 dias");
        assert_eq!(values[3], "671.100 km");
        assert_eq!(values.len(), STAT_LABELS.len());
    }
}
