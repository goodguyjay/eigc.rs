use eigc_terrain::height::slope::SlopeColorField;
use eigc_terrain::height::{ColorSource, HeightSource};

/// Plano inclinado: sobe `slope` metros por metro ao longo de x, constante em z.
struct Ramp {
    slope: f32,
}

impl HeightSource for Ramp {
    fn height_at(&self, x: f32, _z: f32) -> f32 {
        self.slope * x
    }
}

const FLAT: [f32; 4] = [1.0, 1.0, 1.0, 1.0];
const STEEP: [f32; 4] = [0.0, 0.0, 0.0, 1.0];

/// Campo de cor com limiares 0,1 e 0,3 sobre um plano de inclinação `slope`.
fn field_for_slope(slope: f32) -> SlopeColorField<Ramp> {
    SlopeColorField {
        source: Ramp { slope },
        sample_step_m: 10.0,
        flat_color: FLAT,
        steep_color: STEEP,
        slope_start: 0.1,
        slope_end: 0.3,
    }
}

/// Terreno mais suave que o limiar inferior fica com a cor plana.
#[test]
fn gentle_slope_keeps_the_flat_color() {
    assert_eq!(field_for_slope(0.05).color_at(3.0, 4.0), FLAT);
}

/// Terreno mais íngreme que o limiar superior fica totalmente com a cor de encosta.
#[test]
fn steep_slope_gets_the_steep_color() {
    assert_eq!(field_for_slope(0.5).color_at(3.0, 4.0), STEEP);
}

/// Entre os limiares a cor transiciona e o ponto médio é a mistura meio a meio.
#[test]
fn slope_between_thresholds_blends_the_colors() {
    let color = field_for_slope(0.2).color_at(0.0, 0.0);
    for channel in &color[..3] {
        assert!((channel - 0.5).abs() < 1e-4, "esperava 0.5, veio {channel}");
    }
}
