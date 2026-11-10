//! Configuração de ambiente local do desenvolvedor, carregada de um arquivo RON fora do
//! controle de versão.

use std::path::Path;

use bevy::log::warn;
use bevy::prelude::Resource;
use serde::Deserialize;

/// Caminho padrão do arquivo de configuração de ambiente, relativo ao diretório de trabalho em
/// que o binário é executado.
pub const ENVIRONMENT_PATH: &str = "config/environment.ron";

/// Configuração de ambiente local, carregada de `config/environment.ron`. Arquivo ausente ou
/// com conteúdo inválido cai nos valores padrão.
#[derive(Resource, Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct Environment {
    /// Quando verdadeiro, pula a animação de abertura e começa direto no menu.
    pub skip_intro: bool,
}

impl Environment {
    /// Carrega a configuração a partir de `ENVIRONMENT_PATH`.
    pub fn load() -> Self {
        Self::load_from(Path::new(ENVIRONMENT_PATH))
    }

    /// Carrega a configuração de um caminho específico. Arquivo ausente ou inválido resulta nos
    /// valores padrão.
    pub fn load_from(path: &Path) -> Self {
        let Ok(text) = std::fs::read_to_string(path) else {
            return Self::default();
        };

        match Self::parse(&text) {
            Ok(environment) => environment,
            Err(error) => {
                warn!(
                    "{} inválido, usando valores padrão: {error}",
                    path.display()
                );
                Self::default()
            }
        }
    }

    /// Interpreta o conteúdo RON da configuração de ambiente.
    pub fn parse(text: &str) -> Result<Self, ron::error::SpannedError> {
        ron::from_str(text)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    #[rstest]
    #[case("(skip_intro: true)", true)]
    #[case("(skip_intro: false)", false)]
    fn parses_skip_intro(#[case] text: &str, #[case] expected: bool) {
        assert_eq!(Environment::parse(text).unwrap().skip_intro, expected);
    }

    #[test]
    fn rejects_invalid_value() {
        assert!(Environment::parse("(skip_intro: talvez)").is_err());
    }

    #[test]
    fn missing_file_falls_back_to_default() {
        let environment = Environment::load_from(Path::new("nao/existe/environment.ron"));
        assert!(!environment.skip_intro);
    }
}
