//! Testes de integração das paterae: forma da depressão (piso plano, escarpa, sem efeito fora da
//! borda), sorteio determinístico respeitando exclusão e sobreposição, e cor de piso e halo.

use bevy::prelude::Vec2;
use eigc_terrain::height::patera::{
    CarvePaterae, OUTLINE_HARMONICS, PateraColorField, PateraScale, PateraSpec,
    generate_patera_specs,
};
use eigc_terrain::height::{ColorSource, HeightSource};

/// Terreno plano na altura dada.
struct Flat(f32);

impl HeightSource for Flat {
    fn height_at(&self, _x: f32, _z: f32) -> f32 {
        self.0
    }
}

/// Cor constante.
struct Solid([f32; 4]);

impl ColorSource for Solid {
    fn color_at(&self, _x: f32, _z: f32) -> [f32; 4] {
        self.0
    }
}

const BASE: [f32; 4] = [1.0, 1.0, 0.0, 1.0];
const FLOOR: [f32; 4] = [0.0, 0.0, 0.0, 1.0];
const HALO: [f32; 4] = [1.0, 0.0, 0.0, 1.0];

/// Patera circular (sem deformação do contorno) em (2000, -1000), raio 1000 m, 200 m de
/// profundidade e escarpa de 400 m.
fn circular_patera() -> PateraSpec {
    PateraSpec {
        center: Vec2::new(2000.0, -1000.0),
        radius_m: 1000.0,
        depth_m: 200.0,
        scarp_width_m: 400.0,
        outline_amplitudes: [0.0; OUTLINE_HARMONICS],
        outline_phases_rad: [0.0; OUTLINE_HARMONICS],
    }
}

fn scale() -> PateraScale {
    PateraScale {
        radius_min_m: 1200.0,
        radius_max_m: 3000.0,
        depth_min_m: 150.0,
        depth_max_m: 400.0,
        max_scarp_slope: 0.8,
        outline_amplitude_max: 0.08,
        outer_margin_fraction: 0.4,
    }
}

/// Testa que o piso fica plano na altura do terreno ao redor menos a profundidade, e que fora da
/// borda a altura da fonte não muda.
#[test]
fn carved_patera_has_a_flat_floor_and_leaves_the_outside_untouched() {
    let spec = circular_patera();
    let carved = CarvePaterae::new(Flat(30.0), vec![spec]);

    for offset in [Vec2::ZERO, Vec2::new(300.0, 0.0), Vec2::new(-200.0, 350.0)] {
        let p = spec.center + offset;
        assert!(
            (carved.height_at(p.x, p.y) - (30.0 - 200.0)).abs() < 1e-3,
            "piso em {p}"
        );
    }
    for offset in [
        Vec2::new(1000.0, 0.0),
        Vec2::new(0.0, -1500.0),
        Vec2::new(5000.0, 5000.0),
    ] {
        let p = spec.center + offset;
        assert_eq!(carved.height_at(p.x, p.y), 30.0, "fora da borda em {p}");
    }
}

/// Testa que a escarpa desce de forma monotônica da borda até o piso e que a inclinação máxima
/// fica em 1,5 vez a média (perfil em smoothstep), sem degrau.
#[test]
fn scarp_descends_monotonically_with_bounded_slope() {
    let spec = circular_patera();
    let carved = CarvePaterae::new(Flat(0.0), vec![spec]);
    let step = 2.0;
    let mut previous = carved.height_at(spec.center.x + 1000.0, spec.center.y);
    let mut max_slope = 0.0_f32;
    let mut distance = 1000.0 - step;
    while distance >= 500.0 {
        let height = carved.height_at(spec.center.x + distance, spec.center.y);
        assert!(
            height <= previous,
            "a escarpa sobe em {distance} m do centro"
        );
        max_slope = max_slope.max((previous - height) / step);
        previous = height;
        distance -= step;
    }
    let expected = 1.5 * spec.depth_m / spec.scarp_width_m;
    assert!(
        (max_slope - expected).abs() < 0.02,
        "inclinação máxima {max_slope}, esperada {expected}"
    );
}

/// Testa que, onde duas paterae se cruzam, vale a depressão mais funda.
#[test]
fn overlapping_paterae_keep_the_deepest_floor() {
    let shallow = circular_patera();
    let deep = PateraSpec {
        center: shallow.center + Vec2::new(500.0, 0.0),
        depth_m: 350.0,
        ..shallow
    };
    let carved = CarvePaterae::new(Flat(0.0), vec![shallow, deep]);
    let between = shallow.center + Vec2::new(250.0, 0.0);
    assert!((carved.height_at(between.x, between.y) + 350.0).abs() < 1e-3);
}

/// Testa que o sorteio é determinístico, respeita as faixas de medida, a zona de exclusão em
/// volta da origem (com a margem) e não sobrepõe paterae.
#[test]
fn generated_paterae_are_deterministic_and_respect_keep_out_and_spacing() {
    let scale = scale();
    for seed in [1_u32, 7, 99, 501, 31_337] {
        let specs = generate_patera_specs(seed, 11_000.0, 3, 350.0, &scale);
        assert_eq!(
            specs,
            generate_patera_specs(seed, 11_000.0, 3, 350.0, &scale)
        );
        assert!(!specs.is_empty(), "seed {seed}: nenhuma patera");

        let outer = |s: &PateraSpec| s.max_rim_radius() * (1.0 + scale.outer_margin_fraction);
        for (i, spec) in specs.iter().enumerate() {
            assert!((scale.radius_min_m..=scale.radius_max_m).contains(&spec.radius_m));
            assert!((scale.depth_min_m..=scale.depth_max_m).contains(&spec.depth_m));
            assert!(spec.center.x.abs() <= 11_000.0 && spec.center.y.abs() <= 11_000.0);
            assert!(
                spec.center.length() - outer(spec) >= 350.0,
                "seed {seed}: invade a origem"
            );
            assert!(
                spec.scarp_width_m < spec.radius_m * (1.0 - 3.0 * scale.outline_amplitude_max),
                "seed {seed}: escarpa engole o piso"
            );
            for other in &specs[i + 1..] {
                assert!(spec.center.distance(other.center) >= outer(spec) + outer(other));
            }
        }
    }
    assert_ne!(
        generate_patera_specs(1, 11_000.0, 3, 350.0, &scale),
        generate_patera_specs(2, 11_000.0, 3, 350.0, &scale)
    );
}

/// Testa que o contorno deformado fica dentro do raio máximo declarado e varia com o ângulo.
#[test]
fn deformed_outline_stays_within_max_rim_radius() {
    let spec = PateraSpec {
        outline_amplitudes: [0.08, 0.05, 0.03],
        outline_phases_rad: [0.3, 1.7, 4.0],
        ..circular_patera()
    };
    let radii: Vec<f32> = (0..360)
        .map(|deg| spec.rim_radius_at((deg as f32).to_radians()))
        .collect();
    assert!(radii.iter().all(|&r| r <= spec.max_rim_radius() + 1e-3));
    let (min, max) = radii
        .iter()
        .fold((f32::MAX, f32::MIN), |(a, b), &r| (a.min(r), b.max(r)));
    assert!(
        max - min > 100.0,
        "o contorno deveria ser irregular: de {min} a {max}"
    );
}

/// Testa a cor: piso escuro no centro, halo logo fora da borda e cor de fundo longe da patera.
#[test]
fn patera_color_paints_floor_halo_and_leaves_the_rest() {
    let spec = circular_patera();
    let field = PateraColorField {
        base: Solid(BASE),
        paterae: vec![spec],
        floor_color: FLOOR,
        halo_color: HALO,
        halo_width_fraction: 0.5,
    };

    assert_eq!(field.color_at(spec.center.x, spec.center.y), FLOOR);
    let halo = field.color_at(spec.center.x + 1001.0, spec.center.y);
    assert!(
        halo[0] > 0.99 && halo[1] < 0.01,
        "halo logo fora da borda: {halo:?}"
    );
    assert_eq!(field.color_at(spec.center.x + 1600.0, spec.center.y), BASE);
    assert_eq!(field.color_at(-8000.0, 8000.0), BASE);
}
