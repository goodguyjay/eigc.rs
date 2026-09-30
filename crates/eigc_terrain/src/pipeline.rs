//! Pipeline genérico de terreno: registra um Plugin do bevy que spawna a grade de chunks do
//! terreno usando o HeightFn e TerrainParams fornecidos e troca o LOD de cada chunk conforme a
//! distância ao foco.

use crate::chunks::{TerrainLodStats, spawn_terrain_chunks, update_terrain_lod};
use crate::height::{ColorFn, HeightFn};
use crate::lod::TerrainLodConfig;
use crate::params::TerrainParams;
use crate::recipe::build_recipe;
use crate::systems::{TerrainMaterialProperties, build_terrain_material};
use bevy::prelude::{
    App, Assets, Color, Commands, IntoScheduleConfigs, Mesh, OnEnter, Plugin, Res, ResMut,
    Resource, StandardMaterial, Update, in_state, resource_exists,
};
use eigc_common::lod_focus::LodFocus;
use eigc_moons::profile::MoonProfile;
use eigc_moons::{ActiveMoonProfileHandle, AppState};

/// Encapsula a função de altura composta para poder ser usada como recurso no Bevy.
#[derive(Resource, Clone)]
pub struct HeightResource(pub HeightFn);

/// Encapsula a fonte de cor por vértice para poder ser usada como recurso no Bevy. Só é inserido
/// quando a receita da lua produz uma cor (ex.: lineae de Europa).
#[derive(Resource, Clone)]
pub struct ColorResource(pub ColorFn);

/// Estrutura que define a aparência visual do terreno, incluindo cor base e nome de exibição.
#[derive(Resource, Clone)]
pub struct TerrainAppearance {
    /// Cor base do material do terreno.
    pub base_color: Color,
    /// Nome de exibição do terreno, usado para identificação.
    pub display_name: String,
}

/// Plugin de terreno: spawna os chunks ao entrar em `Running` e troca o LOD deles a cada frame.
#[derive(Clone)]
pub struct TerrainPlugin;

/// Implementação do Plugin do bevy para o TerrainPlugin
impl Plugin for TerrainPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<LodFocus>()
            .init_resource::<TerrainLodStats>()
            .add_systems(
                OnEnter(AppState::Running),
                build_terrain_from_loaded_profile,
            )
            .add_systems(
                Update,
                update_terrain_lod
                    .run_if(in_state(AppState::Running))
                    .run_if(resource_exists::<HeightResource>)
                    .run_if(resource_exists::<TerrainParams>)
                    .run_if(resource_exists::<TerrainLodConfig>),
            );
    }
}

/// Lê o perfil de lua ativo, monta a receita de terreno correspondente, spawna os chunks e
/// insere os resources resultantes no Bevy.
fn build_terrain_from_loaded_profile(
    mut commands: Commands,
    active_handle: Res<ActiveMoonProfileHandle>,
    moon_profiles: Res<Assets<MoonProfile>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let Some(profile) = moon_profiles.get(&active_handle.0) else {
        return;
    };

    let recipe = build_recipe(profile);

    // TESTE: cor de vértice é multiplicada pela cor base do material (ver bevy example
    // vertex_colors.rs) — com recipe.color.is_some(), o material precisa ficar branco pra
    // não filtrar a cor de vértice das lineae.
    let base_color = if recipe.color.is_some() {
        Color::WHITE
    } else {
        recipe.appearance.base_color
    };
    let appearance = TerrainAppearance {
        base_color,
        display_name: recipe.appearance.display_name.clone(),
    };

    let material_properties = TerrainMaterialProperties {
        perceptual_roughness: profile.terrain.perceptual_roughness,
        reflectance: profile.terrain.reflectance,
    };

    let material = materials.add(build_terrain_material(&appearance, material_properties));
    spawn_terrain_chunks(
        &mut commands,
        &mut meshes,
        material,
        &recipe.params,
        &recipe.lod,
        recipe.height.as_ref(),
        recipe.color.as_deref(),
    );

    commands.insert_resource(recipe.params);
    commands.insert_resource(recipe.lod);
    if let Some(color) = recipe.color {
        commands.insert_resource(ColorResource(color));
    }
    commands.insert_resource(HeightResource(recipe.height));
    commands.insert_resource(appearance);
    commands.insert_resource(material_properties);
}
