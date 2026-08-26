//! Carregador de asset para arquivos .ron descrevendo MoonProfile.
//! Registra a extensão .ron como reconhecida pelo AssetServer do bevy, permitindo MoonProfile ser
//! carregado como qualquer outro asset do bevy, incluindo hot reload via a feature file_watcher.

use crate::profile::MoonProfile;
use bevy::asset::io::Reader;
use bevy::asset::{AssetLoader, LoadContext};
use bevy::prelude::TypePath;
use thiserror::Error;

#[derive(Default, TypePath)]
pub struct MoonProfileLoader;

/// Erro possível ao carregar um MoonProfile a partir de disco.
#[derive(Debug, Error)]
pub enum MoonProfileLoaderError {
    #[error("Falha ao ler arquivo de perfil de lua: {0}")]
    ReadFailure(#[from] std::io::Error),
    #[error("Falha ao interpretar conteúdo ron de perfil de lua: {0}")]
    ParseFailure(#[from] ron::de::SpannedError),
}

/// Implementação do AssetLoader para MoonProfile, permitindo que arquivos .ron sejam carregados como assets no Bevy.
impl AssetLoader for MoonProfileLoader {
    type Asset = MoonProfile;
    type Settings = ();
    type Error = MoonProfileLoaderError;

    async fn load(
        &self,
        reader: &mut dyn Reader,
        _settings: &Self::Settings,
        _load_context: &mut LoadContext<'_>,
    ) -> Result<Self::Asset, Self::Error> {
        let mut raw_file_contents = Vec::new();
        reader.read_to_end(&mut raw_file_contents).await?;
        let moon_profile = ron::de::from_bytes::<MoonProfile>(&raw_file_contents)?;
        Ok(moon_profile)
    }

    fn extensions(&self) -> &[&str] {
        &["ron"]
    }
}
