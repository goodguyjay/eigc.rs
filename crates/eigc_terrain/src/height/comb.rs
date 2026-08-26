use super::HeightSource;

pub struct Add2<A: HeightSource, B: HeightSource> {
    pub a: A,
    pub b: B,
}
impl<A: HeightSource, B: HeightSource> HeightSource for Add2<A, B> {
    fn height_at(&self, x: f32, z: f32) -> f32 {
        self.a.height_at(x, z) + self.b.height_at(x, z)
    }
}

pub struct Scale<S: HeightSource> {
    pub s: S,
    pub scale: f32,
}
impl<S: HeightSource> HeightSource for Scale<S> {
    fn height_at(&self, x: f32, z: f32) -> f32 {
        self.s.height_at(x, z) * self.scale
    }
}

pub struct Bias<S: HeightSource> {
    pub s: S,
    pub bias: f32,
}
impl<S: HeightSource> HeightSource for Bias<S> {
    fn height_at(&self, x: f32, z: f32) -> f32 {
        self.s.height_at(x, z) + self.bias
    }
}
