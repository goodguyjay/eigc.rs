//! Caminho diário do sol no céu de uma lua em rotação síncrona com Júpiter.
//!
//! Júpiter e o Sol estão (aproximadamente) no plano orbital da lua, então o caminho aparente do
//! sol é um grande círculo que passa pela direção de Júpiter. Todo grande círculo corta o
//! horizonte em dois pontos opostos, o que dá metade do período de dia e metade de noite, e o
//! sol passa atrás de Júpiter uma vez por órbita (eclipse). Aproximação assumida: obliquidade
//! zero, ou seja, plano orbital da lua igual ao plano do Sol (erro real de ~3°).

use bevy::prelude::{Quat, Vec3};

/// Direção normalizada do sol no instante `t` (segundos de simulação).
///
/// Em `t = 0` o sol está no ponto mais alto do círculo (meio-dia, elevação `noon_elevation`),
/// e o ciclo se repete a cada `period` segundos. `noon_elevation` está em radianos.
///
/// O círculo precisa passar por Júpiter, então a elevação de meio-dia não pode ser menor que a
/// elevação de Júpiter. Se for, ela é limitada à elevação de Júpiter (Júpiter vira o meio-dia).
pub fn sun_direction(
    t: f32,
    period: f32,
    jupiter_dir: Vec3,
    up: Vec3,
    noon_elevation: f32,
) -> Vec3 {
    let jupiter = jupiter_dir.normalize();
    let up = up.normalize();

    // Base do plano perpendicular a Júpiter: b1 é a vertical projetada nesse plano, b2 completa.
    let up_in_plane = up.reject_from(jupiter);
    let cos_jupiter_elevation = up_in_plane.length();
    let b1 = up_in_plane
        .try_normalize()
        .unwrap_or_else(|| jupiter.any_orthonormal_vector());
    let b2 = jupiter.cross(b1);

    // O eixo do círculo faz com a vertical um ângulo igual à elevação de meio-dia:
    // eixo·up = cos(alpha)·cos(elev_júpiter) = cos(noon_elevation).
    let cos_alpha = if cos_jupiter_elevation > f32::EPSILON {
        (noon_elevation.cos() / cos_jupiter_elevation).clamp(-1.0, 1.0)
    } else {
        1.0
    };
    let sin_alpha = (1.0 - cos_alpha * cos_alpha).max(0.0).sqrt();
    let axis = (b1 * cos_alpha + b2 * sin_alpha).normalize();

    // Meio-dia: o ponto do círculo mais próximo da vertical.
    let noon = up.reject_from(axis).try_normalize().unwrap_or(jupiter);

    let phase = (t / period) * std::f32::consts::TAU;
    (Quat::from_axis_angle(axis, phase) * noon).normalize()
}

/// Fator de 0.0 (noite) a 1.0 (dia) para a luz do sol, com transição suave entre 2° abaixo e
/// 3° acima do horizonte (crepúsculo).
pub fn horizon_factor(sun_dir: Vec3, up: Vec3) -> f32 {
    let elevation = sun_dir
        .normalize()
        .dot(up.normalize())
        .clamp(-1.0, 1.0)
        .asin();
    eigc_common::math::smoothstep((-2.0_f32).to_radians(), 3.0_f32.to_radians(), elevation)
}

#[cfg(test)]
mod tests {
    use super::{horizon_factor, sun_direction};
    use bevy::prelude::Vec3;
    use rstest::rstest;

    /// Período de Europa em segundos, só para ter um número realista.
    const PERIOD: f32 = 306_822.0;
    /// Direção base de Júpiter calibrada em europa.ron (~14.5° de elevação).
    const JUPITER: Vec3 = Vec3::new(0.35, 0.25, -0.90);
    /// Elevação de meio-dia calibrada em europa.ron.
    const NOON_DEG: f32 = 25.0;

    /// Elevação em graus de uma direção em relação a `Vec3::Y`.
    fn elevation_deg(dir: Vec3) -> f32 {
        dir.normalize().dot(Vec3::Y).asin().to_degrees()
    }

    /// Direção do sol numa fração do dia de europa (0.0 = meio-dia, 0.5 = meia-noite).
    fn sun_at(fraction_of_day: f32) -> Vec3 {
        sun_direction(
            fraction_of_day * PERIOD,
            PERIOD,
            JUPITER,
            Vec3::Y,
            NOON_DEG.to_radians(),
        )
    }

    /// Meio-dia, nascer/pôr e meia-noite: o sol tem que passar por baixo do horizonte, que é
    /// exatamente o que a versão antiga (rotação em torno da vertical) não fazia.
    #[rstest]
    #[case::noon(0.0, NOON_DEG)]
    #[case::sunset_or_sunrise(0.25, 0.0)]
    #[case::midnight(0.5, -NOON_DEG)]
    #[case::sunrise_or_sunset(0.75, 0.0)]
    fn sun_elevation_follows_day_cycle(#[case] fraction_of_day: f32, #[case] expected_deg: f32) {
        let measured = elevation_deg(sun_at(fraction_of_day));
        assert!(
            (measured - expected_deg).abs() < 0.05,
            "fração {fraction_of_day}: esperado {expected_deg}°, medido {measured}°"
        );
    }

    /// O círculo do sol passa por Júpiter, então em algum momento do dia há eclipse.
    #[test]
    fn sun_path_crosses_jupiter_once_per_day() {
        let samples = 3600;
        let closest_deg = (0..samples)
            .map(|i| {
                sun_at(i as f32 / samples as f32)
                    .angle_between(JUPITER.normalize())
                    .to_degrees()
            })
            .fold(f32::MAX, f32::min);
        assert!(
            closest_deg < 0.5,
            "o sol deveria passar por Júpiter, mas chega no máximo a {closest_deg}°"
        );
    }

    /// Depois de um período inteiro o sol volta à mesma direção.
    #[test]
    fn sun_path_closes_after_one_period() {
        let start = sun_at(0.0);
        let end = sun_at(1.0);
        assert!((start - end).length() < 1e-3, "início {start}, fim {end}");
    }

    /// Elevação de meio-dia abaixo da elevação de Júpiter não tem solução exata: não pode dar
    /// panic nem NaN.
    #[test]
    fn noon_below_jupiter_elevation_stays_finite_and_normalized() {
        for i in 0..8 {
            let dir = sun_direction(
                i as f32 * PERIOD / 8.0,
                PERIOD,
                JUPITER,
                Vec3::Y,
                5.0_f32.to_radians(),
            );
            assert!(dir.is_finite(), "direção não finita: {dir}");
            assert!((dir.length() - 1.0).abs() < 1e-5, "não normalizada: {dir}");
        }
    }

    /// A luz é total de dia e nula à noite.
    #[rstest]
    #[case::day(0.0, 1.0)]
    #[case::night(0.5, 0.0)]
    fn horizon_factor_is_full_by_day_and_zero_at_night(
        #[case] fraction_of_day: f32,
        #[case] expected: f32,
    ) {
        let factor = horizon_factor(sun_at(fraction_of_day), Vec3::Y);
        assert!(
            (factor - expected).abs() < 1e-5,
            "fração {fraction_of_day}: esperado {expected}, obtido {factor}"
        );
    }

    /// No horizonte exato (dentro da faixa de crepúsculo) o fator fica entre 0 e 1.
    #[test]
    fn horizon_factor_is_partial_at_twilight() {
        let factor = horizon_factor(Vec3::X, Vec3::Y);
        assert!(
            factor > 0.0 && factor < 1.0,
            "crepúsculo deveria ser parcial: {factor}"
        );
    }
}
