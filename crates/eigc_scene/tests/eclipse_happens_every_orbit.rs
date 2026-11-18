//! Garante que as calibrações de céu reais de Io e Ganimedes produzem um eclipse por Júpiter em
//! toda órbita, com duração próxima da real.

use bevy::MinimalPlugins;
use bevy::prelude::{App, Update};
use eigc_moons::{MoonId, MoonProfile};
use eigc_scene::sky::{SkySettings, SkyState, animate_sky_physical};
use eigc_sim::SimTime;
use std::path::PathBuf;

/// Órbitas simuladas. A libração tem período de algumas órbitas, então quatro cobrem o pior caso
/// de afastamento entre a trajetória do sol e o centro de Júpiter.
const ORBITS: usize = 4;

/// Amostras por órbita (passo de 0,5 grau de fase).
const SAMPLES_PER_ORBIT: usize = 720;

/// Fração mínima de cada órbita com o sol fora de Júpiter (pode estar abaixo do horizonte).
const MIN_FULL_SUN_FRACTION: f32 = 0.85;

/// Lê e desserializa o perfil real da lua a partir da pasta de assets do workspace.
fn real_profile(moon_id: MoonId) -> MoonProfile {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../assets")
        .join(moon_id.profile_asset_path());
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("não consegui ler {}: {error}", path.display()));
    ron::de::from_str(&text)
        .unwrap_or_else(|error| panic!("o .ron de {moon_id:?} não é um MoonProfile válido: {error}"))
}

/// Simula `ORBITS` órbitas da lua e confere, em cada uma, que a escuridão total do eclipse dura
/// entre `min_hours` e `max_hours`, e que fora dele o sol fica inteiro a maior parte do tempo.
fn assert_total_eclipse_every_orbit(moon_id: MoonId, min_hours: f32, max_hours: f32) {
    let settings = SkySettings::from_profile(&real_profile(moon_id));
    let period = settings.orbital_period_seconds;
    let step_seconds = period / SAMPLES_PER_ORBIT as f32;

    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .insert_resource(settings)
        .init_resource::<SkyState>()
        .insert_resource(SimTime(0.0))
        .add_systems(Update, animate_sky_physical);

    for orbit in 0..ORBITS {
        let mut total_eclipse_samples = 0;
        let mut full_sun_samples = 0;

        for sample in 0..SAMPLES_PER_ORBIT {
            app.world_mut().resource_mut::<SimTime>().0 =
                (orbit * SAMPLES_PER_ORBIT + sample) as f32 * step_seconds;
            app.update();

            let eclipse_factor = app.world().resource::<SkyState>().eclipse_factor;
            if eclipse_factor <= 1e-4 {
                total_eclipse_samples += 1;
            }
            if eclipse_factor >= 1.0 - 1e-4 {
                full_sun_samples += 1;
            }
        }

        let total_eclipse_hours = total_eclipse_samples as f32 * step_seconds / 3600.0;
        assert!(
            (min_hours..=max_hours).contains(&total_eclipse_hours),
            "{moon_id:?}, órbita {orbit}: eclipse total durou {total_eclipse_hours:.2} h, \
             esperado entre {min_hours} e {max_hours} h"
        );

        let full_sun_fraction = full_sun_samples as f32 / SAMPLES_PER_ORBIT as f32;
        assert!(
            full_sun_fraction >= MIN_FULL_SUN_FRACTION,
            "{moon_id:?}, órbita {orbit}: sol inteiro em só {:.1}% da órbita",
            full_sun_fraction * 100.0
        );
    }
}

/// Testa que toda órbita de Io tem eclipse total com duração realista (o real fica perto de
/// 2h15).
#[test]
fn io_has_a_total_eclipse_of_realistic_duration_every_orbit() {
    assert_total_eclipse_every_orbit(MoonId::Io, 2.0, 3.0);
}

/// Testa que toda órbita de Ganimedes tem eclipse total com duração realista. Com Júpiter menor
/// no céu e a órbita mais lenta, o eclipse dura mais que o de Io (o real chega a ~3,5 h).
#[test]
fn ganymede_has_a_total_eclipse_of_realistic_duration_every_orbit() {
    assert_total_eclipse_every_orbit(MoonId::Ganymede, 2.5, 3.6);
}
