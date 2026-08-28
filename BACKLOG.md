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
- [ ] Câmera e luz em eigc_app::scene_placeholder são fixas e hardcoded,
  sem calibração por lua. migrar para eigc_scene quando esse crate for
  desenhado (céu, luz solar orientada por MoonProfile, câmera dupla
  andar/órbita)

## Arquitetura aceita
- TerrainPlugin acopla carregamento de asset (`Handle<MoonProfile>`) com
  construção de terreno, em vez de separar as duas responsabilidades.
  decisão consciente para reduzir refactor agora; revisar só se esse
  acoplamento virar dor real. Provavelmente não, mas sei lá.