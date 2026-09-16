# Débito técnico e pendências conhecidas

## Terreno
- [ ] Calibrar receita de terreno de io (`recipe.rs`, hoje unimplemented!)
- [ ] Calibrar receita de terreno de ganimedes (`recipe.rs`, hoje unimplemented!)
- [ ] Calibrar receita de terreno de calisto (`recipe.rs`, hoje unimplemented!)
- [X] Revisar bias: -0.1 na receita de Europa (`recipe.rs`), sem justificativa
  documentada de motivo geológico ou estético
- [ ] `TerrainParams.amp` não é mais a fonte de verdade da escala vertical do terreno.
  Ao implementar lineae (issue de terreno reconhecível de Europa), descobrimos que
  `Scale { scale: 1.0 }` em `europa_recipe` ignorava `vertical_amplitude_meters`, foi
  corrigido usando `calibration.vertical_amplitude_meters` diretamente dentro da receita,
  não via `TerrainParams.amp` (que segue sem uso real no pipeline, só testado). Duplicação
  de estado consciente, não resolvida agora. `amp` deveria ser removido ou passar a ser
  a única fonte de verdade quando outra lua for calibrada.
- [ ] **Ainda não totalmente resolvido**: a exclusão acima impede uma linea de nascer
  *ancorada* dentro do raio, mas não impede que o **segmento** de uma linea vizinha ainda
  alcance a zona de blend. Como a orientação de toda linea é ~paralela a `feature_direction`
  (mesmo eixo usado pra alinhar a grade), uma linea na célula logo ao lado da excluída, com
  comprimento longo o bastante (protagonistas chegam a ~5600m, metade = ~2800m de cada lado
  do centro), ainda pode ultrapassar a origem. Quando isso acontece, `FlattenNearOrigin`
  ainda atenua a altura dessa crista de forma radialmente simétrica em torno da origem (`d =
  sqrt(x²+z²)`, não a distância à própria linea), o que pode ler como a crista "convergindo"
  pro centro. Resolver de vez exigiria testar interseção do traçado inteiro (segmento/arco)
  contra o disco de exclusão, não só a distância do centro. Não fizemos isso ainda.
- [ ] **criar issue separada.** A câmera free-fly spawna em posição mundial
  fixa (`Vec3::new(0.0, 600.0, 1200.0)`, `eigc_scene::camera::free_fly_camera::
  spawn_free_fly_camera`), sem nenhuma leitura do `HeightResource` do terreno e sem
  coincidir com o centro (`x=0, z=0`) onde fica a clareira de spawn (`SPAWN_CLEARING_RADIUS_M`
  em `eigc_terrain::recipe`), o `z=1200` da câmera cai fora do raio de ~350m da clareira.
  Não é bloqueante hoje (a câmera só não fica dentro do relevo por sorte de escala: altura
  máxima física agora limitada a ~300m pelo fix de `LineaField::height_at` abaixo), mas o
  deve ser resolvido/revisitado numa issue própria, fazendo possivelmente a câmera
  nascer dentro da clareira e/ou ler a altura real do terreno no ponto de spawn.
- [X] **Crateras desligadas (feedback visual: terreno "artificial", alturas
  empilhadas, crateras em lugares estranhos).** Não existe um sistema de "cratera" dedicado
  no projeto, o que lia como cratera era o ruído `PerlinRidged`/`Oriented` (`ridged_noise`/
  `oriented_ridges`/`subtle_ridges`) somado ao relevo base em `europa_recipe`, mesmo rebaixado
  a `scale: 0.15` na iteração anterior. Comentado (não removido) em `recipe.rs`. Para religar,
  descomentar o bloco, voltar a importar `PerlinRidged` (`height::noise`) e `Oriented`
  (`height::warp`), e trocar `combined_features` de volta pra
  `Add2 { a: base_noise, b: subtle_ridges }`.
- [ ] Traçado em arco de linea (`LineaPath::Arc`, `height/linea.rs`) é uma aproximação
  geométrica estilizada e deliberadamente exagerada, não um modelo físico da tensão de
  maré real. Meramente estético/artístico.

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