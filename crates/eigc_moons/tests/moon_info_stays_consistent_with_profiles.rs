//! Guarda de consistência entre `MoonInfo` (tabela estática de apresentação) e os perfis `.ron`
//! reais em `assets/moons/`. Três dados existem nos dois lugares de propósito (ver `BACKLOG.md`):
//! nome, disponibilidade (`available` / `walkable`) e período orbital. Se um lado mudar sem o
//! outro, estes testes quebram, em vez de a divergência passar em silêncio.

use eigc_moons::*;
use std::path::PathBuf;

/// Segundos em um dia terrestre, para comparar o período do `.ron` com o do `MoonInfo`.
const SECONDS_PER_DAY: f64 = 86_400.0;

/// Diferença relativa aceita entre os dois períodos: `MoonInfo` guarda o valor arredondado.
const PERIOD_TOLERANCE: f64 = 0.005;

/// Lê o `.ron` de perfil da lua a partir da pasta de assets do workspace.
fn profile_text(moon_id: MoonId) -> String {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../assets")
        .join(moon_id.profile_asset_path());
    std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("não consegui ler {}: {error}", path.display()))
}

/// Desserializa o perfil de uma lua que tem `.ron` preenchido.
fn parse_profile(moon_id: MoonId) -> MoonProfile {
    ron::de::from_str(&profile_text(moon_id)).unwrap_or_else(|error| {
        panic!("o .ron de {moon_id:?} não é um MoonProfile válido: {error}")
    })
}

/// Testa que toda lua marcada como disponível no `MoonInfo` tem o `.ron` de
/// perfil preenchido.
#[test]
fn moon_marked_available_has_a_filled_profile() {
    for moon_id in MOON_DISPLAY_ORDER {
        if moon_info(moon_id).available {
            assert!(
                !profile_text(moon_id).trim().is_empty(),
                "{moon_id:?} está `available` no MoonInfo, mas o .ron do perfil está vazio"
            );
        }
    }
}

/// Testa que, para toda lua com `.ron` preenchido, nome, `walkable` e período
/// orbital batem com o `MoonInfo` (o período com tolerância de 0,5%).
#[test]
fn name_availability_and_period_match_every_filled_profile() {
    for moon_id in MOON_DISPLAY_ORDER {
        if profile_text(moon_id).trim().is_empty() {
            continue;
        }
        let info = moon_info(moon_id);
        let profile = parse_profile(moon_id);

        assert_eq!(
            profile.moon_id, moon_id,
            "moon_id do .ron difere do esperado"
        );
        assert_eq!(
            profile.display_name, info.display_name,
            "display_name diverge entre o .ron e o MoonInfo de {moon_id:?}"
        );
        assert_eq!(
            profile.walkable, info.available,
            "`walkable` do .ron e `available` do MoonInfo divergem em {moon_id:?}"
        );

        let ron_period_days = f64::from(profile.sky.orbital_period_seconds) / SECONDS_PER_DAY;
        let relative_difference =
            ((ron_period_days - f64::from(info.orbital_period_days)) / ron_period_days).abs();
        assert!(
            relative_difference < PERIOD_TOLERANCE,
            "período orbital de {moon_id:?} diverge: .ron = {ron_period_days:.4} dias, \
             MoonInfo = {} dias",
            info.orbital_period_days
        );
    }
}

/// Diferença relativa aceita entre `jupiter_angular_diameter_deg` e o dobro de
/// `jupiter_ang_radius`: os dois são escritos à mão no `.ron`, com arredondamento.
const JUPITER_SIZE_TOLERANCE: f32 = 0.001;

/// Testa que, em todo `.ron` preenchido, o diâmetro angular de Júpiter em graus e o raio
/// angular em radianos descrevem o mesmo tamanho. São a mesma grandeza guardada duas vezes no
/// perfil, e uma pode mudar sem a outra.
#[test]
fn jupiter_angular_diameter_and_radius_agree_in_every_filled_profile() {
    for moon_id in MOON_DISPLAY_ORDER {
        if profile_text(moon_id).trim().is_empty() {
            continue;
        }
        let profile = parse_profile(moon_id);
        let diameter_from_radius_deg = (2.0 * profile.sky.jupiter_ang_radius).to_degrees();
        let relative_difference = ((diameter_from_radius_deg
            - profile.jupiter_angular_diameter_deg)
            / profile.jupiter_angular_diameter_deg)
            .abs();
        assert!(
            relative_difference < JUPITER_SIZE_TOLERANCE,
            "tamanho de Júpiter diverge em {moon_id:?}: {} graus no diâmetro, {} graus pelo raio",
            profile.jupiter_angular_diameter_deg,
            diameter_from_radius_deg
        );
    }
}
