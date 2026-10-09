# CLAUDE.md — eigc_scene

Câmera (free-fly, camera lock sol/Júpiter) e céu (sol, Júpiter, starfield, planetshine, eclipse). Depende de `eigc_moons` e `eigc_terrain` — nunca o contrário. Se uma mudança aqui parecer exigir que `eigc_moons` ou `eigc_terrain` passem a depender de `eigc_scene`, pare e pergunte ao desenvolvedor humano antes de prosseguir; é sinal de lógica no crate errado, não de dependência circular legítima.

## Histórico de bugs neste crate — contexto que não está no CLAUDE.md da raiz

O porte de `CamLock`/`LockMode` (módulo `camera::free_fly_camera`) já revelou uma sequência de bugs sutis, todos de dessincronia de estado, não de lógica óbvia errada:

- **`CamLock` nunca era inserido como resource** — causava panic silencioso até ser rastreado.
- **`base_sun_dir` e `base_jupiter_dir` trocados** dentro de `resolve_lock_direction` — sintoma era Júpiter parecendo funcionar (libração pequena, erro pouco visível) enquanto o sol travava errado.
- **Yaw/pitch dessincronizavam ao sair do lock.** Corrigido extraindo o ângulo via `to_euler(EulerRot::YXZ)` — não improvise outra ordem de euler aqui sem entender por que essa foi a que funcionou.
- **`CamLock` lia de `SkySettings.base_sun_dir` (valor estático calibrado uma vez) em vez de `SkyState.sun_dir` (direção animada por frame).** Esse é o caso canônico de duplicação de estado do projeto: `SkySettings` e `SkyState` representam a mesma grandeza física em momentos diferentes do pipeline, e é fácil ler a fonte errada sem erro de compilação nem panic — só o comportamento fica sutilmente errado.

Se você for tocar em qualquer sistema que leia direção de sol, Júpiter, ou orientação de câmera, primeiro identifique se a fonte é `SkySettings` (estático, calibração) ou `SkyState` (dinâmico, atualizado por `animate_sky_physical`). Ler a errada não quebra a build.

## Ordenação de sistema

`lock_aim_update` tem `.run_if(resource_exists::<SkySettings>)` e depende de rodar depois de `animate_sky_physical` — isso é garantido via `.chain()`, não por sorte de ordem de registro. Se adicionar um novo sistema nesse pipeline (sky → camera lock → o que vier depois), ele entra na cadeia explícita, não solto num `add_systems` separado.

## Testes deste crate

Além da convenção geral (nome de arquivo por comportamento observável), este crate tem duas categorias que merecem teste separado:
- Função pura extraída de sistema (`clamp_pitch`, `resolve_lock_direction`) — teste direto, sem ECS.
- Regressão de bug de estado (ex.: o caso `SkySettings` vs `SkyState`) — teste que expressa o comportamento errado que já aconteceu, não só o comportamento correto genérico. Um teste que só verifica "a câmera trava" não pegaria a dessincronia; precisa verificar que a direção usada é a dinâmica.

## Pendências conhecidas deste crate (ver `BACKLOG.md` para o texto completo)

- Damping/smoothing na transição de lock — ainda não implementado, aceito conscientemente por ora.
- `mouse_look` não é totalmente desabilitado durante o lock.
- ESC não pausa `mouse_look` depois do cursor ser liberado.
- Markers de componente privados (`Jupiter`, `SunDisc`, `StarDome`) ainda sem teste de integração.

## Limite de DirectionalLight (risco monitorado, não mais implícito)

O Bevy 0.18 suporta no máximo `MAX_DIRECTIONAL_LIGHTS = 10` (constante interna de
`bevy_pbr::render::light`, não pública fora do crate). Hoje o app produz exatamente 2:
`SunLight` (`sky::sun`) e `PlanetShine` (`sky::planet_shine`). Havia uma terceira, vestigial,
em `eigc_app::scene_placeholder`, que foi removida por não ter mais função depois do `SkyPlugin`
existir. `crates/eigc_scene/tests/directional_light_budget.rs` monta o pipeline real (headless)
e falha se esse número mudar sem atualização deliberada do teste — não adicione uma nova
`DirectionalLight` permanente sem revisar esse teste.

Não resolva essas pendências de forma incidental enquanto faz outra tarefa neste crate — se esbarrar em uma delas, confirme com o desenvolvedor humano antes de decidir se agora é a hora de resolver ou só documentar que ainda está pendente.