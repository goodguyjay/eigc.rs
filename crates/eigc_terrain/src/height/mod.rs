use std::sync::Arc;

pub mod comb;
pub mod noise;
pub mod warp;

/// Estrutura que representa uma fonte de altura para o terreno.
/// Implementações dessa trait devem fornecer um método para obter a altura em coordenadas (x, z).
pub trait HeightSource: Send + Sync + 'static {
    fn height_at(&self, x: f32, z: f32) -> f32;
}

/// Tipo de função que representa uma fonte de altura compartilhada entre threads.
pub type HeightFn = Arc<dyn HeightSource>;

/// Função auxiliar para ar uma fonte de altura compartilhada a partir de uma implementação de HeightSource.
pub fn arc<S: HeightSource>(s: S) -> HeightFn {
    Arc::new(s)
}
