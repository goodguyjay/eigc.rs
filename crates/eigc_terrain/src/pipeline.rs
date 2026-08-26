//! Pipeline genérico de terreno: registra um Plugin do bevy que spawna uma única
//! entidade de malha de terreno usando o HeightFn e TerrainParams fornecidos.

use crate::height::HeightFn;
use crate::recipe::build_recipe;
use crate::systems::build_and_spawn_terrain;
use bevy::prelude::{
    App, Assets, Color, Commands, Mesh, OnEnter, Plugin, Res, ResMut, Resource, StandardMaterial,
};
use eigc_moons::profile::MoonProfile;
use eigc_moons::{ActiveMoonProfileHandle, AppState};

/// Encapsula a função de altura composta para poder ser usada como recurso no Bevy.
#[derive(Resource, Clone)]
pub struct HeightResource(pub HeightFn);

/// Estrutura que define a aparência visual do terreno, incluindo cor base e nome de exibição.
#[derive(Resource, Clone)]
pub struct TerrainAppearance {
    /// Cor base do material do terreno.
    pub base_color: Color,
    /// Nome de exibição do terreno, usado para identificação.
    pub display_name: String,
}

/// Estrutura principal do plugin de terreno, contendo os parâmetros, função de altura e aparência do terreno.
#[derive(Clone)]
pub struct TerrainPlugin;

/// Implementação do Plugin do bevy para o TerrainPlugin
impl Plugin for TerrainPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            OnEnter(AppState::Running),
            build_terrain_from_loaded_profile,
        );
    }
}

/// Lê o perfil de lua ativo, monta a receita de terreno correspondente, spawna a entidade e insere
/// os resources resultantes no Bevy.
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

    let appearance = TerrainAppearance {
        base_color: recipe.appearance.base_color,
        display_name: recipe.appearance.display_name.clone(),
    };

    build_and_spawn_terrain(
        &mut commands,
        &mut meshes,
        &mut materials,
        recipe.params,
        recipe.height.as_ref(),
        &appearance,
    );

    commands.insert_resource(recipe.params);
    commands.insert_resource(HeightResource(recipe.height));
    commands.insert_resource(appearance);
}
