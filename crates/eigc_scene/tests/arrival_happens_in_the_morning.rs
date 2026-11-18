//! Garante que, nas luas com céu de lugar real (Io e Ganimedes), a simulação começa de manhã:
//! sol acima do horizonte e subindo, longe do eclipse. Sem isso, a chegada pode cair no meio da
//! noite, com o chão escuro.

use bevy::MinimalPlugins;
use bevy::prelude::{App, Update};
use eigc_moons::{MoonId, MoonProfile};
use eigc_scene::sky::{SkySettings, SkyState, animate_sky_physical};
use eigc_sim::SimTime;
use std::path::PathBuf;

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

/// Estado do céu da lua no instante `t` (segundos de simulação).
fn sky_at(app: &mut App, t: f32) -> SkyState {
    app.world_mut().resource_mut::<SimTime>().0 = t;
    app.update();
    *app.world().resource::<SkyState>()
}

/// Testa que Io e Ganimedes começam com o sol entre 20 e 40 graus, subindo, com luz do dia e sem
/// eclipse.
#[test]
fn site_moons_start_in_the_morning_with_the_sun_rising() {
    for moon_id in [MoonId::Io, MoonId::Ganymede] {
        let settings = SkySettings::from_profile(&real_profile(moon_id));
        let an_hour_later = 3600.0;

        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .insert_resource(settings)
            .init_resource::<SkyState>()
            .insert_resource(SimTime(0.0))
            .add_systems(Update, animate_sky_physical);

        let start = sky_at(&mut app, 0.0);
        let later = sky_at(&mut app, an_hour_later);
        let elevation_deg = start.sun_dir.y.asin().to_degrees();

        assert!(
            (20.0..=40.0).contains(&elevation_deg),
            "{moon_id:?}: sol a {elevation_deg:.1} graus na chegada"
        );
        assert!(later.sun_dir.y > start.sun_dir.y, "{moon_id:?}: o sol deveria estar subindo");
        assert_eq!(start.daylight_factor, 1.0, "{moon_id:?}");
        assert_eq!(start.eclipse_factor, 1.0, "{moon_id:?}");
    }
}
