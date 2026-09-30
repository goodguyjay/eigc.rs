//! Geração procedural de terreno, kit-of-parts por lua.

/// Construção da malha de um chunk do terreno (grade, normais, skirt).
pub mod chunk_mesh;
/// Componentes e sistemas dos chunks do terreno (spawn inicial e troca de nível de LOD).
pub mod chunks;
/// Fontes de altura componíveis (`HeightSource`) usadas para gerar o relevo do terreno.
pub mod height;
/// Configuração e matemática pura do LOD por chunks (grade, distância, escolha de nível).
pub mod lod;
/// Construção da malha 3D do terreno a partir de uma fonte de altura.
pub mod mesh;
/// Parâmetros de geração do terreno (dimensões, resolução, etc.).
pub mod params;
/// Sistemas Bevy que spawnam e atualizam o terreno.
pub mod systems;
/// Plugin e recursos do pipeline de terreno.
pub mod pipeline;
/// Receitas de composição de fontes de altura por lua.
pub mod recipe;