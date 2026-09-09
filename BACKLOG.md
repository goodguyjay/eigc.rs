# Débito técnico e pendências conhecidas

## Terreno
- [ ] Calibrar receita de terreno de io (`recipe.rs`, hoje unimplemented!)
- [ ] Calibrar receita de terreno de ganimedes (`recipe.rs`, hoje unimplemented!)
- [ ] Calibrar receita de terreno de calisto (`recipe.rs`, hoje unimplemented!)
- [ ] Revisar bias: -0.1 na receita de Europa (`recipe.rs`), sem justificativa
  documentada de motivo geológico ou estético
- [X] `perceptual_roughness` e `reflectance` em `systems.rs::spawn_terrain` estão
  chumbados (0.7 / 0.5), deveriam vir de `MoonProfile` por lua (gelo de
  Europa reflete diferente de enxofre de Io) (issue #5)

## Cena / Visual
- [X] Câmera e luz em eigc_app::scene_placeholder eram fixas e hardcoded, sem
  calibração por lua. A luz placeholder (`spawn_placeholder_light`) foi
  removida — o céu real (`SkyPlugin`, com `SunLight` e `PlanetShine`) já
  cobre esse papel e a luz vestigial estava empilhando com as luzes de
  verdade, quase estourando o limite de `DirectionalLight` do Bevy
  (`MAX_DIRECTIONAL_LIGHTS = 10`). Guarda de regressão adicionada em
  `crates/eigc_scene/tests/directional_light_budget.rs`. A parte de câmera
  dupla andar/órbita continua pendente, não fazia parte desse fix.
- [ ] Jupiter está com orientação incorreta no céu
- [ ] falta damping/smoothing na transição, e não desabilitar mouse_look completamente durante o lock

## Arquitetura aceita
- TerrainPlugin acopla carregamento de asset (`Handle<MoonProfile>`) com
  construção de terreno, em vez de separar as duas responsabilidades.
  decisão consciente para reduzir refactor agora; revisar só se esse
  acoplamento virar dor real. Provavelmente não, mas sei lá.

## Acoplamento geral
onde: `sky::animate_sky_physical`, `sky::jupiter::place_and_scale_jupiter`, 
`sky::sun::position_sun_disc`, `sky::starfield::dim_stars_near_sun`.

Essas quatro funções concentram lógica matemática não trivial (libração orbital, cálculo de 
eclipse via smoothstep, posicionamento/escala angular de disco celeste, composição de dois 
smoothstep para o glare de estrelas) diretamente como Bevy systems, recebendo `Query/Res/ResMut`
do ECS. Diferente de `sky::shared::place_celestial_disc`, que já isola a matemática numa função pura (Vec3/f32 in, 
Vec3/f32 out), essas quatro não tiveram a mesma extração.

por quê foi aceito assim por agora: extrair cada uma para uma função pura testável sem ECS
exigiria redesenhar a assinatura dos quatro systems (transformá-los em wrappers finos 
que só leem `Query/Res`, chamam a função pura, e escrevem o resultado de volta), o que 
é retrabalho não trivial em cima de código que acabou de ser portado e validado.

o trade-off aceito: cobertura vem via teste de integração (montar App mínimo, rodar app.update(), inspecionar o 
resource/transform resultante), não via teste unitário isolado de função pura.

## Plataformas
- Em `camera.rs` o comportamento de `CursorGrabMode::Locked´ não é garantido em todas as plataformas. macOS
e X11 não possuem suporte completo e o bevy pode recair silenciosamente para `CursorGrabMode::Confined`.
**todo (jay): vê isso depois Rodger**