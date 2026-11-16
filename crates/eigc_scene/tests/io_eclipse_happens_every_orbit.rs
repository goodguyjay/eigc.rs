//! Garante que a calibração de céu do `io.ron` real produz um eclipse por Júpiter em toda órbita,
//! com duração próxima da real (cerca de 2h15 a 2h20).

use bevy::MinimalPlugins;
use bevy::prelude::{App, Update};
use eigc_moons::{MoonId, MoonProfile};
use eigc_scene::sky::{SkySettings, SkyState, animate_sky_physical};
use eigc_sim::SimTime;
use std::path::PathBuf;

/// Órbitas simuladas. A libração em latitude tem período de ~3,33 órbitas, então quatro cobrem
/// o pior caso de afastamento entre a trajetória do sol e o centro de Júpiter.
const ORBITS: usize = 4;

/// Amostras por órbita (passo de 0,5 grau de fase).
const SAMPLES_PER_ORBIT: usize = 720;

/// Faixa aceita para a duração da escuridão total, em horas.
const MIN_TOTAL_ECLIPSE_HOURS: f32 = 2.0;
const MAX_TOTAL_ECLIPSE_HOURS: f32 = 3.0;

/// Fração mínima de cada órbita com o sol totalmente visível.
const MIN_FULL_SUN_FRACTION: f32 = 0.85;

/// Lê e desserializa o perfil real de Io a partir da pasta de assets do workspace.
fn io_profile() -> MoonProfile {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../assets")
        .join(MoonId::Io.profile_asset_path());
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("não consegui ler {}: {error}", path.display()));
    ron::de::from_str(&text)
        .unwrap_or_else(|error| panic!("o io.ron não é um MoonProfile válido: {error}"))
}

/// Testa que toda órbita de Io tem eclipse total com duração realista, e que fora dele o sol
/// fica totalmente visível na maior parte do tempo.
#[test]
fn io_has_a_total_eclipse_of_realistic_duration_every_orbit() {
    let settings = SkySettings::from_profile(&io_profile());
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
            (MIN_TOTAL_ECLIPSE_HOURS..=MAX_TOTAL_ECLIPSE_HOURS).contains(&total_eclipse_hours),
            "órbita {orbit}: eclipse total durou {total_eclipse_hours:.2} h, esperado entre \
             {MIN_TOTAL_ECLIPSE_HOURS} e {MAX_TOTAL_ECLIPSE_HOURS} h"
        );

        let full_sun_fraction = full_sun_samples as f32 / SAMPLES_PER_ORBIT as f32;
        assert!(
            full_sun_fraction >= MIN_FULL_SUN_FRACTION,
            "órbita {orbit}: sol totalmente visível em só {:.1}% da órbita",
            full_sun_fraction * 100.0
        );
    }
}
