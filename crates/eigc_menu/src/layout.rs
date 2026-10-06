//! Funções puras de disposição das luas na cena 3D do menu: escala relativa e posições.

use bevy::prelude::Vec3;
use eigc_moons::{MOON_DISPLAY_ORDER, MoonId, moon_info};

/// Raio, em unidades de mundo, da esfera dentro dos arquivos `.glb` das luas.
const GLB_SPHERE_RADIUS: f32 = 500.0;

/// Raio, em unidades de mundo do menu, da maior lua. As demais são proporcionais a ela.
const LARGEST_MOON_WORLD_RADIUS: f32 = 2.0;

/// Distância entre os centros de duas luas vizinhas na fileira do menu.
const MOON_SPACING: f32 = 6.0;

/// Distância da câmera ao plano das luas na visão geral, escolhida para enquadrar a fileira inteira.
const OVERVIEW_CAMERA_DISTANCE: f32 = 20.0;

/// Distância da câmera à lua em foco, medida em raios da própria lua.
const FOCUS_DISTANCE_IN_RADII: f32 = 5.0;

/// Deslocamento lateral da câmera em foco, como fração da distância, para a lua ficar à esquerda
/// do centro e sobrar espaço para o painel de detalhes à direita.
const FOCUS_SIDE_SHIFT: f32 = 0.25;

/// Ampliação aplicada à lua sob o cursor ou em foco.
const ACTIVE_SCALE_BOOST: f32 = 0.06;

/// Distância do rótulo ao centro da lua, em raios da lua, já contando a ampliação por hover.
const ACTIVE_LABEL_GAP_IN_RADII: f32 = 1.25;

/// Posição da câmera na visão geral. A câmera nunca gira: só se desloca entre visão geral e foco.
pub(crate) fn overview_camera_position() -> Vec3 {
    Vec3::new(0.0, 0.0, OVERVIEW_CAMERA_DISTANCE)
}

/// Posição alvo da câmera: visão geral sem foco, ou aproximada da lua em foco.
pub(crate) fn camera_target(focus: Option<MoonId>) -> Vec3 {
    let Some(moon_id) = focus else {
        return overview_camera_position();
    };
    let world_radius = moon_uniform_scale(moon_id) * GLB_SPHERE_RADIUS;
    let distance = world_radius * FOCUS_DISTANCE_IN_RADII;
    let moon = moon_position(moon_id);
    Vec3::new(
        moon.x + distance * FOCUS_SIDE_SHIFT,
        moon.y,
        moon.z + distance,
    )
}

/// Multiplicador de escala de uma lua, maior quando ela está sob o cursor ou em foco.
pub(crate) fn active_scale_factor(is_active: bool) -> f32 {
    if is_active {
        1.0 + ACTIVE_SCALE_BOOST
    } else {
        1.0
    }
}

/// Aproxima `current` de `target` com amortecimento exponencial, independente da taxa de quadros.
pub(crate) fn damp_towards(current: Vec3, target: Vec3, rate: f32, delta_secs: f32) -> Vec3 {
    target + (current - target) * (-rate * delta_secs).exp()
}

/// Versão escalar de `damp_towards`.
pub(crate) fn damp_f32(current: f32, target: f32, rate: f32, delta_secs: f32) -> f32 {
    target + (current - target) * (-rate * delta_secs).exp()
}

/// Ponto do mundo, logo abaixo da lua, onde o rótulo da visão geral é ancorado.
pub(crate) fn label_anchor_world(moon_id: MoonId) -> Vec3 {
    let world_radius = moon_uniform_scale(moon_id) * GLB_SPHERE_RADIUS;
    moon_position(moon_id) - Vec3::Y * (world_radius * ACTIVE_LABEL_GAP_IN_RADII)
}

/// Escala uniforme a aplicar ao modelo `.glb` da lua para que ela apareça no tamanho relativo real.
pub(crate) fn moon_uniform_scale(moon_id: MoonId) -> f32 {
    let largest_radius_km = MOON_DISPLAY_ORDER
        .iter()
        .map(|id| moon_info(*id).radius_km())
        .fold(0.0_f32, f32::max);
    LARGEST_MOON_WORLD_RADIUS * (moon_info(moon_id).radius_km() / largest_radius_km)
        / GLB_SPHERE_RADIUS
}

/// Posição do centro da lua na fileira, centralizada na origem e ordenada em `MOON_DISPLAY_ORDER`.
pub(crate) fn moon_position(moon_id: MoonId) -> Vec3 {
    let index = MOON_DISPLAY_ORDER
        .iter()
        .position(|id| *id == moon_id)
        .unwrap_or(0);
    let centered_index = index as f32 - (MOON_DISPLAY_ORDER.len() as f32 - 1.0) / 2.0;
    Vec3::new(centered_index * MOON_SPACING, 0.0, 0.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Testa que, sem lua em foco, a câmera tem como alvo a posição da visão
    /// geral.
    #[test]
    fn camera_without_focus_stays_in_overview_position() {
        assert_eq!(camera_target(None), overview_camera_position());
    }

    /// Testa que, com qualquer lua em foco, a câmera chega mais perto que na
    /// visão geral e fica deslocada para a direita da lua, deixando-a à
    /// esquerda do centro.
    #[test]
    fn focused_camera_gets_closer_than_overview_and_keeps_moon_left_of_center() {
        for moon_id in MOON_DISPLAY_ORDER {
            let target = camera_target(Some(moon_id));
            assert!(target.z < overview_camera_position().z);
            assert!(target.x > moon_position(moon_id).x);
        }
    }

    /// Testa que a distância da câmera em foco é proporcional ao tamanho da
    /// lua: Europa, menor, fica mais perto que Ganimedes.
    #[test]
    fn focus_distance_is_proportional_to_moon_size() {
        let europa = camera_target(Some(MoonId::Europa)).z;
        let ganymede = camera_target(Some(MoonId::Ganymede)).z;
        assert!(europa < ganymede);
    }

    /// Testa que a lua sob o cursor ou em foco é ampliada, e que a lua em
    /// repouso mantém a escala 1.
    #[test]
    fn active_moon_is_scaled_up() {
        assert!(active_scale_factor(true) > active_scale_factor(false));
        assert_eq!(active_scale_factor(false), 1.0);
    }

    /// Testa que o amortecimento converge para o alvo em 10 s a 60 quadros por
    /// segundo, sem nunca o ultrapassar (a distância ao alvo só diminui).
    #[test]
    fn damping_converges_to_target_without_overshoot() {
        let target = Vec3::new(4.0, 0.0, 2.0);
        let mut current = Vec3::ZERO;
        for _ in 0..600 {
            let next = damp_towards(current, target, 6.0, 1.0 / 60.0);
            assert!(next.distance(target) <= current.distance(target));
            current = next;
        }
        assert!(current.distance(target) < 1e-3);
    }

    /// Testa que o ponto de ancoragem do rótulo fica abaixo da lua, na mesma
    /// coluna, e mais distante quanto maior é a lua.
    #[test]
    fn label_sits_below_its_moon_and_scales_with_moon_size() {
        let europa_gap =
            moon_position(MoonId::Europa).y - label_anchor_world(MoonId::Europa).y;
        let ganymede_gap =
            moon_position(MoonId::Ganymede).y - label_anchor_world(MoonId::Ganymede).y;
        assert!(europa_gap > 0.0);
        assert!(europa_gap < ganymede_gap);
        assert_eq!(
            super::label_anchor_world(MoonId::Io).x,
            moon_position(MoonId::Io).x
        );
    }

    /// Testa que a versão escalar do amortecimento dá o mesmo resultado que a
    /// vetorial.
    #[test]
    fn scalar_damping_matches_vector_damping() {
        let vector = damp_towards(Vec3::new(3.0, 0.0, 0.0), Vec3::ZERO, 5.0, 0.1);
        assert!((damp_f32(3.0, 0.0, 5.0, 0.1) - vector.x).abs() < 1e-6);
    }

    /// Testa que um quadro de duração zero não move o valor amortecido.
    #[test]
    fn damping_with_zero_delta_does_not_move() {
        let current = Vec3::new(1.0, 2.0, 3.0);
        assert_eq!(damp_towards(current, Vec3::ZERO, 6.0, 0.0), current);
    }

    /// Testa que a maior lua (Ganimedes) recebe exatamente o raio de referência
    /// do menu.
    #[test]
    fn largest_moon_gets_the_reference_world_radius() {
        let world_radius = moon_uniform_scale(MoonId::Ganymede) * GLB_SPHERE_RADIUS;
        assert!((world_radius - LARGEST_MOON_WORLD_RADIUS).abs() < 1e-5);
    }

    /// Testa que a escala segue a ordem real dos raios: Europa, Io, Calisto e
    /// Ganimedes, do menor para o maior.
    #[test]
    fn smaller_moons_have_smaller_scale() {
        assert!(moon_uniform_scale(MoonId::Europa) < moon_uniform_scale(MoonId::Io));
        assert!(moon_uniform_scale(MoonId::Io) < moon_uniform_scale(MoonId::Callisto));
        assert!(moon_uniform_scale(MoonId::Callisto) < moon_uniform_scale(MoonId::Ganymede));
    }

    /// Testa que as luas ficam em fileira, da esquerda para a direita, na ordem
    /// de exibição.
    #[test]
    fn moons_are_laid_out_left_to_right_in_display_order() {
        let xs: Vec<f32> = MOON_DISPLAY_ORDER
            .iter()
            .map(|id| moon_position(*id).x)
            .collect();
        assert!(xs.windows(2).all(|pair| pair[0] < pair[1]));
    }

    /// Testa que a fileira de luas é centralizada na origem.
    #[test]
    fn row_is_centered_on_the_origin() {
        let sum: f32 = MOON_DISPLAY_ORDER
            .iter()
            .map(|id| moon_position(*id).x)
            .sum();
        assert!(sum.abs() < 1e-5);
    }
}
