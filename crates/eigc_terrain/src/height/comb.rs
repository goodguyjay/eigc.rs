//! Combinadores de `HeightSource` (soma, escala, viés) e `ColorSource`.

use super::{ColorSource, HeightSource};
use eigc_common::math::{lerp4, smoothstep};

/// Soma a altura de duas fontes de altura.
pub struct Add2<A: HeightSource, B: HeightSource> {
    /// Primeira fonte de altura somada.
    pub a: A,
    /// Segunda fonte de altura somada.
    pub b: B,
}
impl<A: HeightSource, B: HeightSource> HeightSource for Add2<A, B> {
    fn height_at(&self, x: f32, z: f32) -> f32 {
        self.a.height_at(x, z) + self.b.height_at(x, z)
    }
}

/// Multiplica a altura de uma fonte por um fator de escala.
pub struct Scale<S: HeightSource> {
    /// Fonte de altura escalada.
    pub s: S,
    /// Fator multiplicativo aplicado à altura.
    pub scale: f32,
}
impl<S: HeightSource> HeightSource for Scale<S> {
    fn height_at(&self, x: f32, z: f32) -> f32 {
        self.s.height_at(x, z) * self.scale
    }
}

/// Soma um deslocamento constante (viés) à altura de uma fonte.
pub struct Bias<S: HeightSource> {
    /// Fonte de altura deslocada.
    pub s: S,
    /// Deslocamento constante somado à altura.
    pub bias: f32,
}
impl<S: HeightSource> HeightSource for Bias<S> {
    fn height_at(&self, x: f32, z: f32) -> f32 {
        self.s.height_at(x, z) + self.bias
    }
}

/// Achata a altura numa clareira circular ao redor da origem (x=0, z=0), com transição suave até
/// a altura completa da fonte fora do raio de blend. Usada para garantir um ponto de spawn plano
/// e legível, evitando relevo caótico ou entidades (câmera, jogador) nascendo dentro do terreno.
pub struct FlattenNearOrigin<S: HeightSource> {
    /// Fonte de altura completa, usada fora da clareira.
    pub source: S,
    /// Altura constante dentro da clareira, em metros.
    pub flat_height: f32,
    /// Raio da clareira totalmente plana, em metros.
    pub flat_radius_m: f32,
    /// Distância adicional, além de `flat_radius_m`, em que a altura transiciona suavemente da
    /// clareira até a fonte completa.
    pub blend_radius_m: f32,
}
impl<S: HeightSource> FlattenNearOrigin<S> {
    /// Número de pontos amostrados ao redor do perímetro onde o blend termina, pra calcular
    /// `flat_height` em `new`. 16 é arbitrário.
    const BOUNDARY_SAMPLE_COUNT: usize = 16;

    /// Cria uma clareira cuja altura plana é a média da altura real da fonte ao longo do
    /// perímetro onde o blend termina (`flat_radius_m + blend_radius_m`), em vez de um valor
    /// fixo arbitrário.
    pub fn new(source: S, flat_radius_m: f32, blend_radius_m: f32) -> Self {
        let sample_radius = flat_radius_m + blend_radius_m;
        let sum: f32 = (0..Self::BOUNDARY_SAMPLE_COUNT)
            .map(|i| {
                let angle = (i as f32 / Self::BOUNDARY_SAMPLE_COUNT as f32) * std::f32::consts::TAU;
                source.height_at(angle.cos() * sample_radius, angle.sin() * sample_radius)
            })
            .sum();
        let flat_height = sum / Self::BOUNDARY_SAMPLE_COUNT as f32;

        Self {
            source,
            flat_height,
            flat_radius_m,
            blend_radius_m,
        }
    }
}
impl<S: HeightSource> HeightSource for FlattenNearOrigin<S> {
    fn height_at(&self, x: f32, z: f32) -> f32 {
        let d = (x * x + z * z).sqrt();
        if d <= self.flat_radius_m {
            return self.flat_height;
        }
        let full = self.source.height_at(x, z);
        let blend_end = self.flat_radius_m + self.blend_radius_m;
        if d >= blend_end {
            return full;
        }
        let t = smoothstep(self.flat_radius_m, blend_end, d);
        self.flat_height + (full - self.flat_height) * t
    }
}

/// Faz a altura descer até um piso plano perto da borda de um disco centrado na origem. Dentro de
/// `fade_start_radius_m` devolve a fonte intacta, entre `fade_start_radius_m` e
/// `fade_end_radius_m` desce suavemente (smoothstep) até `floor_height` e, de `fade_end_radius_m`
/// para fora, devolve o piso sem consultar a fonte. Deixa o terreno com cara de disco.
///
/// O piso deve ficar abaixo do ponto mais baixo da fonte dentro de `fade_start_radius_m`; se
/// ficar acima, a rampa sobe em vez de descer.
pub struct FadeToFloorBeyondRadius<S: HeightSource> {
    /// Fonte de altura completa, usada dentro do raio interno.
    pub source: S,
    /// Altura constante fora do raio externo, em metros.
    pub floor_height: f32,
    /// Raio, em metros, até onde a fonte vale intacta.
    pub fade_start_radius_m: f32,
    /// Raio, em metros, a partir do qual a altura é o piso.
    pub fade_end_radius_m: f32,
}
impl<S: HeightSource> HeightSource for FadeToFloorBeyondRadius<S> {
    fn height_at(&self, x: f32, z: f32) -> f32 {
        let d = (x * x + z * z).sqrt();
        if d >= self.fade_end_radius_m {
            return self.floor_height;
        }
        let full = self.source.height_at(x, z);
        if d <= self.fade_start_radius_m {
            return full;
        }
        let t = smoothstep(self.fade_start_radius_m, self.fade_end_radius_m, d);
        full + (self.floor_height - full) * t
    }
}

/// Equivalente a `FlattenNearOrigin`, mas para `ColorSource`: substitui a cor por uma cor fixa
/// dentro da clareira, com a mesma transição suave até a cor original da fonte.
pub struct FlattenColorNearOrigin<S: ColorSource> {
    /// Fonte de cor completa, usada fora da clareira.
    pub source: S,
    /// Cor constante dentro da clareira (RGBA linear).
    pub flat_color: [f32; 4],
    /// Raio da clareira totalmente plana, em metros.
    pub flat_radius_m: f32,
    /// Distância adicional, além de `flat_radius_m`, em que a cor transiciona suavemente da
    /// clareira até a fonte completa.
    pub blend_radius_m: f32,
}
impl<S: ColorSource> ColorSource for FlattenColorNearOrigin<S> {
    fn color_at(&self, x: f32, z: f32) -> [f32; 4] {
        let d = (x * x + z * z).sqrt();
        if d <= self.flat_radius_m {
            return self.flat_color;
        }
        let full = self.source.color_at(x, z);
        let blend_end = self.flat_radius_m + self.blend_radius_m;
        if d >= blend_end {
            return full;
        }
        let t = smoothstep(self.flat_radius_m, blend_end, d);
        lerp4(self.flat_color, full, t)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct ConstHeight(f32);
    impl HeightSource for ConstHeight {
        fn height_at(&self, _x: f32, _z: f32) -> f32 {
            self.0
        }
    }

    #[test]
    fn flatten_near_origin_is_constant_inside_flat_radius() {
        let flatten = FlattenNearOrigin {
            source: ConstHeight(500.0),
            flat_height: 0.0,
            flat_radius_m: 150.0,
            blend_radius_m: 200.0,
        };

        assert_eq!(flatten.height_at(0.0, 0.0), 0.0);
        assert_eq!(flatten.height_at(100.0, 0.0), 0.0);
        assert_eq!(flatten.height_at(0.0, -149.0), 0.0);
    }

    struct LinearHeightAlongX;
    impl HeightSource for LinearHeightAlongX {
        fn height_at(&self, x: f32, _z: f32) -> f32 {
            x
        }
    }

    #[test]
    fn new_derives_flat_height_from_average_around_blend_boundary() {
        // altura varia linearmente com x (positiva de um lado do círculo, negativa do outro);
        // a média ao redor do perímetro inteiro deve cancelar pra ~0, não pegar o valor de um
        // único ponto arbitrário do perímetro.
        let flatten = FlattenNearOrigin::new(LinearHeightAlongX, 150.0, 200.0);

        assert!(
            flatten.flat_height.abs() < 1e-3,
            "altura plana deveria ser a média ao redor do perímetro (~0), foi {}",
            flatten.flat_height
        );
    }

    #[test]
    fn new_matches_surrounding_level_when_source_is_uniformly_elevated() {
        // com uma fonte de altura constante e não-zero, a clareira deveria assumir esse nível em
        // vez de forçar 0.0 fixo.
        let flatten = FlattenNearOrigin::new(ConstHeight(42.0), 150.0, 200.0);

        assert!((flatten.flat_height - 42.0).abs() < 1e-3);
        assert_eq!(flatten.height_at(0.0, 0.0), 42.0);
    }

    #[test]
    fn flatten_near_origin_matches_source_beyond_blend_end() {
        let flatten = FlattenNearOrigin {
            source: ConstHeight(500.0),
            flat_height: 0.0,
            flat_radius_m: 150.0,
            blend_radius_m: 200.0,
        };

        assert_eq!(flatten.height_at(400.0, 0.0), 500.0);
    }

    /// Fonte constante em 500 m com rampa de 1000 m a 2000 m de raio até o piso em -300 m.
    fn edge_fade() -> FadeToFloorBeyondRadius<ConstHeight> {
        FadeToFloorBeyondRadius {
            source: ConstHeight(500.0),
            floor_height: -300.0,
            fade_start_radius_m: 1000.0,
            fade_end_radius_m: 2000.0,
        }
    }

    #[test]
    fn fade_to_floor_keeps_the_source_inside_the_start_radius() {
        let fade = edge_fade();

        assert_eq!(fade.height_at(0.0, 0.0), 500.0);
        assert_eq!(fade.height_at(999.0, 0.0), 500.0);
        assert_eq!(fade.height_at(0.0, -1000.0), 500.0);
    }

    #[test]
    fn fade_to_floor_is_the_floor_at_and_beyond_the_end_radius() {
        let fade = edge_fade();

        assert_eq!(fade.height_at(2000.0, 0.0), -300.0);
        assert_eq!(fade.height_at(0.0, 5000.0), -300.0);
        assert_eq!(fade.height_at(9000.0, -9000.0), -300.0);
    }

    #[test]
    fn fade_to_floor_is_continuous_at_both_radii() {
        let fade = edge_fade();

        assert!((fade.height_at(1000.01, 0.0) - 500.0).abs() < 0.01);
        assert!((fade.height_at(1999.99, 0.0) - -300.0).abs() < 0.01);
    }

    #[test]
    fn fade_to_floor_descends_monotonically_without_leaving_the_range() {
        let fade = edge_fade();

        let mut previous = fade.height_at(1000.0, 0.0);
        for radius in 1001..=2000 {
            let height = fade.height_at(radius as f32, 0.0);
            assert!(
                height <= previous,
                "a rampa subiu em {radius} m: {previous} -> {height}"
            );
            assert!((-300.0..=500.0).contains(&height));
            previous = height;
        }
    }

    #[test]
    fn fade_to_floor_depends_only_on_the_distance_to_the_origin() {
        let fade = edge_fade();
        let radius = 1500.0_f32;
        let along_x = fade.height_at(radius, 0.0);

        for angle in [0.4_f32, 1.1, 2.5, 4.0] {
            let height = fade.height_at(angle.cos() * radius, angle.sin() * radius);
            assert!(
                (height - along_x).abs() < 0.05,
                "ângulo {angle}: {height} contra {along_x}"
            );
        }
    }

    #[test]
    fn flatten_near_origin_blends_monotonically_between_radii() {
        let flatten = FlattenNearOrigin {
            source: ConstHeight(500.0),
            flat_height: 0.0,
            flat_radius_m: 150.0,
            blend_radius_m: 200.0,
        };

        let a = flatten.height_at(200.0, 0.0);
        let b = flatten.height_at(300.0, 0.0);
        assert!(a > 0.0 && a < 500.0, "deveria estar em transição: {a}");
        assert!(b > a && b < 500.0, "deveria continuar subindo em direção à fonte: {b}");
    }
}
