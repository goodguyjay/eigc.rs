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
- [ ] **LOD do terreno usa skirts, não stitching.** Chunks vizinhos de níveis diferentes têm
  vértices de borda diferentes (o grosso é subconjunto do fino), então a emenda é escondida por
  um anel de skirt (`chunk_mesh.rs`, `skirt_depth_factor` em `TerrainLodConfig`) e não por
  costura exata de triângulos. Trade-off: implementação simples e testável, ao custo de uma
  face vertical visível se a diferença de altura entre níveis passar da profundidade do skirt
  (mais provável em silhueta contra o horizonte). Revisitar se aparecer rachadura em teste
  visual, ou trocar por stitching.
- [ ] **Iluminação levemente descontínua entre níveis de LOD.** As normais de cada chunk vêm de
  diferenças centrais no espaçamento do próprio nível, então a normal de um vértice de borda
  difere um pouco entre o chunk fino e o grosso. Dentro do mesmo nível não há costura (o
  cálculo amostra um anel extra além do chunk). Uma alternativa é calcular todas as normais
  com um epsilon fixo (o espaçamento do nível 0), ao custo de 4 amostras de altura extras por
  vértice.
- [ ] **Nível de LOD vem do erro geométrico medido, projetado na tela.** Cada chunk tem o desvio
  vertical de cada nível medido em segundo plano (`lod_error.rs`) e o nível sai de
  `erro * escala_de_tela / distância <= max_screen_error_px`. A escala de tela vem do FOV e da
  altura da janela (`LodFocus.screen_scale`). Limitações conscientes: (1) o erro só considera
  altura, não a cor por vértice, que também perde resolução ao engrossar; o piso
  `min_error_fraction` cobre isso de forma grosseira; (2) o erro é tratado como se fosse visto
  de lado, o que é conservador para câmera alta olhando para baixo; (3) a medição amostra a grade
  do nível 0, então detalhe abaixo de ~10 m não entra no erro. `max_screen_error_px`,
  `min_error_fraction`, `in_flight_vertex_budget` e a resolução por nível (`europa_recipe`) são
  chutes calibráveis, medidos só em teste headless, não com o app rodando.
- [ ] **Sem teto de vértices no LOD.** Como o nível depende do erro em pixels, tolerância baixa
  ou janela muito grande (2160p dobra a escala de tela) aumentam bastante os vértices (teste
  headless com Europa: ~0,7 M a 1080p, ~1,5 M a 2160p). Se precisar, adicionar um teto total de
  vértices que relaxa a tolerância.
- [ ] `TerrainParams.res` só vale para a malha monolítica legada (`build_terrain_mesh`,
  `spawn_terrain`, usadas em testes). O pipeline com chunks usa `TerrainLodConfig`. Mesma
  família da duplicação de `amp` acima: não removido agora porque quebraria os literais de
  `TerrainParams` em vários testes de integração.
- [ ] LOD não considera se o chunk está dentro do frustum (chunks atrás da câmera são
  refinados igual aos da frente, para a câmera poder girar sem ver terreno grosso), não há
  culling por oclusão, e não existe colisão (qualquer modo de caminhada futuro deve ler
  `HeightResource::height_at` direto, não a malha visível).

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
eclipse via smoothstep, orientação/rotação de disco celeste, composição de dois 
smoothstep para o glare de estrelas) diretamente como Bevy systems, recebendo `Query/Res/ResMut`
do ECS.

**Atualização**: `place_and_scale_jupiter` e `position_sun_disc` já tiveram a parte de
*posição e escala* extraída para `sky::shared::place_celestial_disc` (função pura, testada
isoladamente em `shared.rs`) — não são mais "quatro funções sem nenhuma extração". O que ainda
fica inline em cada uma: a orientação (`look_to`, tilt de obliquidade de Júpiter, rotação de
correção `-FRAC_PI_2` do PR #33) e, no caso de Júpiter, a leitura de `MoonProfile`/
`ActiveMoonProfileHandle`. `animate_sky_physical` e `dim_stars_near_sun` continuam sem nenhuma
extração.

por quê foi aceito assim por agora: extrair a parte de orientação para uma função pura testável
sem ECS exigiria redesenhar a assinatura dos systems (transformá-los em wrappers finos 
que só leem `Query/Res`, chamam a função pura, e escrevem o resultado de volta), o que 
é retrabalho não trivial em cima de código que acabou de ser portado e validado.

o trade-off aceito: cobertura da orientação vem via teste de integração (montar App mínimo, rodar
`app.update()`, inspecionar `Transform::forward()`/`Transform::up()` resultante — ver
`sky::sun::tests::position_sun_disc_matches_place_celestial_disc_formula` e
`sky::jupiter::tests::place_and_scale_jupiter_matches_place_celestial_disc_formula`), não via
teste unitário isolado de função pura. Posição/escala, por outro lado, já tem cobertura de função
pura (`sky::shared::tests`), já que passaram a chamar `place_celestial_disc`.

## Plataformas
- Em `camera.rs` o comportamento de `CursorGrabMode::Locked´ não é garantido em todas as plataformas. macOS
e X11 não possuem suporte completo e o bevy pode recair silenciosamente para `CursorGrabMode::Confined`.
**todo (jay): vê isso depois Rodger**