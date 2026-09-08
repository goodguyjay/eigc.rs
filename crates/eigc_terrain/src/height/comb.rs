//! Combinadores de `HeightSource` (soma, escala, viés).

use super::HeightSource;

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
