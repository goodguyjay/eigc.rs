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
- [ ] A câmera free-fly spawna em posição mundial fixa
  (`SPAWN_POSITION` em `eigc_scene::camera::free_fly_camera`, hoje `(0.0, 0.3, 0.0)`, no centro
  da clareira de spawn), sem nenhuma leitura do `HeightResource` do terreno. O `y = 0.3` vem de
  medir o platô da clareira em Europa (y = -1,45 m, a ~1,75 m de altura de olho): se a receita
  mudar o DTM, o `SPAWN_CLEARING_RADIUS_M` ou a elevação de referência, a câmera pode nascer
  dentro do relevo ou flutuando.
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
- [ ] **Heightmap de Europa embutido no binário (`include_bytes!` em `recipe.rs`).** Os
  arquivos continuam em `assets/terrain/europa/` (uma única cópia dos dados), mas são lidos
  em tempo de compilação. Motivo: `build_recipe` é síncrona e pura, chamada em testes sem
  nenhum caminho de assets configurado, e o caminho de assets em runtime é relativo ao
  manifest do binário (`eigc_app`), que `eigc_terrain` não conhece. Custo: sem hot reload
  do heightmap (ao contrário do `MoonProfile`) e ~40 KB a mais no binário por lua. Se mais
  luas ganharem heightmap ou o tamanho crescer, migrar para carregar via `AssetServer`.
- [ ] **Constantes visuais do relevo híbrido de Europa não foram calibradas com o app
  rodando.** `DETAIL_AMPLITUDE_M`, `DETAIL_FREQUENCY_FACTOR` e
  os limiares `SLOPE_COLOR_START/END` em `recipe.rs` são valores de partida, não resultado de
  inspeção visual sistemática. A interpolação já é bicúbica (Catmull-Rom) em
  `HeightmapHeight::height_at`: a bilinear inicial deixava o terreno em tabuleiro (gradiente
  descontínuo a cada pixel de 227,65 m, visível nas normais e na cor por inclinação). A
  Catmull-Rom pode ter pequeno overshoot além da faixa de elevação do DTM, aceito.
  `VERTICAL_EXAGGERATION` ficou em 1.0 (elevação medida) por decisão consciente. O relevo
  parece baixo de longe porque o dado é suave: precisão vertical efetiva de 36 a 100 m RMS,
  feições identificáveis de ~450 a 700 m e declive mediano de 2,5 graus (máximo 21,6). Subir o
  exagero é uma escolha visual, não uma correção física, e exigiria rever o `y` de spawn da câmera contra o relevo
  da clareira.
- [ ] **Código de lineae sintéticas ficou sem uso em Europa.** `height::linea`
  (`LineaField`, `LineaColorField`, `generate_linea_specs`) e `height::warp` não são mais
  chamados por `europa_recipe`, já que as cristas agora vêm do DTM. Também ficaram sem
  efeito para Europa `warp_amplitude_meters`, `feature_direction` e (já era o caso)
  `vertical_amplitude_meters`, que só chega em `TerrainParams.amp`. Mantidos porque outra
  lua pode reaproveitar as lineae e remover campos de `TerrainCalibration` mexe no `.ron` e
  nos testes de `eigc_moons`. O destaque das cristas reais é só por inclinação
  (`SlopeColorField`), sem identificar cada crista individualmente.

- [X] **Textura de superfície de Europa (`snow_02`) sem mipmaps.** O loader de JPEG do Bevy
  não gera mipmaps e o tile físico é de 2 m (`TILE_SIZE_M` em `chunk_mesh.rs`), o que deixava o
  chão como ruído de sal e pimenta. Resolvido com geração de mips em CPU depois do carregamento
  (`surface_mips.rs`: sRGB em espaço linear, normal map renormalizado) e anisotropia 16 no
  sampler. Trade-off: custo único de CPU na thread principal ao entrar em `Running` e mips que
  não persistem em disco. Se incomodar, trocar por KTX2 pré-mipado. Ainda avaliar tile
  repetitivo de perto e blend de baixa frequência de cor para quebrar a repetição.
- [ ] **Mistura de duas escalas do chão de Europa (`terrain_detail.wgsl`) deixa a repetição do
  tile longo visível de cima.** O chão é um `ExtendedMaterial` (`surface_material.rs`): a escala
  curta é o tile físico de 2 m e a longa é o mesmo conjunto de texturas a 16 m
  (`MACRO_SCALE_RATIO`), misturadas por distância (`BLEND_START_M`/`BLEND_END_M`, 4 a 30 m).
  De perto (centímetros) fica como antes, e à altura dos olhos o detalhe longe melhorou, mas a
  30 m de altura o padrão de 16 m aparece em grade. Trade-off: reusar as mesmas imagens evita
  baixar mais assets e mantém a cor coerente, ao custo da repetição. Mitigar com uma terceira
  escala (ex.: 64 m), rotação/offset por camada ou uma máscara de baixa frequência. Os valores
  de escala e de distância são de partida, ajustar visualmente. O shader é carregado de
  `assets/shaders/` em runtime (não embutido), e `TerrainPlugin` só registra o `MaterialPlugin`
  quando existe `RenderApp` (testes headless só registram o asset).
- [ ] **UVs em metros perdem precisão longe da origem.** `uv = world / 2.0` chega a ~5500 nas
  bordas do recorte de 22 km; em `f32` isso dá ~0,0005 de passo, cerca de 1 texel num tile de
  2048 px. Aceito por ora (não observado como artefato). Se aparecer tremor de textura, passar a
  UV relativa à origem do chunk com offset por material.
- [ ] **A máscara de cor das cristas de Europa é inclinação do DTM, não a máscara de linea.** As
  lineae sintéticas não participam mais da altura (ver item acima), então o tingimento por
  vértice reaproveita o `SlopeColorField` (mesma fonte de altura do relevo) com
  `terrain_base_color` branco e `terrain_valley_color` acastanhado em `europa.ron`. O nome
  `terrain_valley_color` ficou impreciso: hoje é a cor das encostas íngremes. Renomear mexe no
  `.ron` e nos testes de `eigc_moons`.

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
- [ ] `place_and_scale_jupiter`, `track_camera` e `dim_stars_near_sun` leem `SkyState` sem
  ordem garantida em relação a `animate_sky_physical`, que o escreve todo frame. Nenhum dos
  três está em `SimSet::Animate` nem usa `.after(...)`.

## Arquitetura aceita
- TerrainPlugin acopla carregamento de asset (`Handle<MoonProfile>`) com
  construção de terreno, em vez de separar as duas responsabilidades.
  decisão consciente para reduzir refactor agora; revisar só se esse
  acoplamento virar dor real. Provavelmente não, mas sei lá.

## Acoplamento geral
onde: `sky::animate_sky_physical`, `sky::jupiter::place_and_scale_jupiter`, 
`sky::sun::position_sun_disc`, `sky::starfield::dim_stars_near_sun`.

Essas funções concentram lógica matemática não trivial (libração orbital, eclipse via
smoothstep, orientação de disco celeste, glare de estrelas) direto em Bevy systems.

`place_and_scale_jupiter` e `position_sun_disc` já usam `place_celestial_disc` para posição
e escala; a orientação continua inline. `animate_sky_physical` e `dim_stars_near_sun` seguem
sem extração.

Motivo: extrair exigiria transformar os systems em wrappers finos, retrabalho grande em
código recém-validado.

Trade-off: a orientação é coberta por teste de integração (App mínimo + `app.update()`),
não por teste unitário de função pura.

## Menu inicial
- [ ] **Voltar ao menu a partir da simulação não existe.** O menu só aparece na entrada
  (`AppState::MainMenu` é o estado inicial e nada leva de volta a ele). Foi cortado de propósito
  do escopo: voltar exigiria `DespawnOnExit` em terreno, céu e câmera, reset de todos os
  resources da simulação (`ChunkErrorTable`, `SimTime`, `SkySettings`, `SkyAssets`,
  `GlobalAmbientLight` etc.) e mover o spawn de `PlanetShineLight` do `Startup` para
  `OnEnter(Running)`. Sem isso, reentrar em `Running` duplicaria Sol, Júpiter e estrelas e
  estouraria o teste de orçamento de `DirectionalLight`. Trade-off: troca de lua exige reiniciar
  o app.
- [ ] `transition_when_moon_profile_loaded` (`eigc_app::moon_loading`) não trata
  `LoadState::Failed`. Se o `.ron` da lua escolhida falhar ao carregar, o app fica preso em
  `LoadingMoonProfile` com a tela de carregamento. Hoje só Europa é selecionável e o RON dela é
  válido, então não dispara. Passa a importar quando as outras luas forem liberadas.
- [ ] **`MoonInfo` (`eigc_moons::moon_info`) duplica três dados do `MoonProfile`.** São eles o
  nome (`display_name`), a disponibilidade (`available` vs `walkable`) e o período orbital
  (`orbital_period_days` vs `sky.orbital_period_seconds`, o segundo com o valor exato, o primeiro
  arredondado). Diâmetro, gravidade, distância de Júpiter e textos existem só no `MoonInfo`, então
  não duplicam. É duplicação consciente, aceita por três motivos:
  1. Os `.ron` de Io, Ganimedes e Calisto estão vazios (0 bytes), e o menu precisa mostrar as
     quatro luas na partida, sem esperar o carregamento assíncrono de perfis que falhariam no
     parse.
  2. O `MoonProfile` não tem campos de apresentação (descrição, estatísticas). Acrescentá-los
     exigiria RON válido para as três luas não calibradas, o que quebra o loader e os testes que
     esperam `unimplemented!()` em `build_recipe` para elas.
  3. Tabela estática em código foi a escolha explícita do desenvolvedor para o menu.
  Mitigação parcial: `eigc_moons/tests/moon_info_stays_consistent_with_profiles.rs`
  falha se nome, `available`/`walkable` ou período (tolerância de 0,5%) divergirem em qualquer lua
  com `.ron` preenchido, e se uma lua `available` tiver `.ron` vazio. Não resolve a causa. Unificar,
  com o menu lendo o perfil, quando os `.ron` das outras luas existirem. Ao liberar uma lua, mudar
  o `MoonInfo` e o `.ron` juntos.
- [ ] **Textos e números de `MoonInfo` foram escritos sem fonte citada** (diâmetro, gravidade,
  período orbital, distância de Júpiter e os resumos). Valores de ordem de grandeza correta, mas
  precisam ser conferidos contra NASA/USGS antes de a ficha ser considerada final.
- [ ] As teclas 0 a 4 de `TimeFlow` (`handle_time_flow_keyboard_controls`, `eigc_sim`) ainda
  respondem no menu e na tela de carregamento, mudando `time_scale` antes da simulação. O
  sistema é privado e não está em nenhum `SimSet`, então o gate de estado de `eigc_app` não o
  alcança. Gatear exigiria que `eigc_sim` conhecesse `AppState` ou que o sistema entrasse num
  `SimSet`. Efeito é só de conveniência (a escolha do jogador é mantida).
- [ ] **Sem transições suaves no menu.** O painel de detalhes desliza, mas título, rótulos e
  aviso aparecem e somem de uma vez. O Bevy 0.18 não tem opacidade de grupo; fazer fade exigiria
  animar o alfa de cada fundo, borda e texto.
- [ ] As texturas dos modelos das luas (`assets/moons/textures/*.glb`) de Europa e Io têm
  4096x2048 e ocupam ~32 MB de VRAM cada depois de descomprimidas (RGBA8), independente do
  formato no disco. Considerar KTX2 ou redução de resolução.
- [ ] A tela de carregamento é estática. O terreno é construído em `OnEnter(Running)`, então a
  tela provavelmente congela por um instante na transição. Não medido; confirmar em teste
  visual antes de investir num indicador animado.

## Plataformas
- Em `camera.rs` o comportamento de `CursorGrabMode::Locked´ não é garantido em todas as plataformas. macOS
e X11 não possuem suporte completo e o bevy pode recair silenciosamente para `CursorGrabMode::Confined`.
**todo (jay): vê isso depois Rodger**