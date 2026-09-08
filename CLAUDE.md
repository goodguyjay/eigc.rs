# CLAUDE.md — eigc-rs

Simulação interativa em Rust/Bevy das quatro luas galileanas de Júpiter (Europa, Io, Ganimedes, Calisto). Workspace Cargo multi-crate. Bevy fixado em **0.18.1** — não subir de versão sem decisão explícita do desenvolvedor humano.

## Aviso importante: Bevy quebra API com frequência

Bevy 0.18.1 é rápido em desenvolvimento, quebra API entre versões com frequência e tem poucos exemplos oficiais atualizados. Qualquer tarefa que toque em código Bevy (sistemas, componentes, plugins, queries, asset loading) envolve bastante guesswork de sua parte. O risco real é gerar código que não compila e estourar tokens tentando corrigir às cegas.

Por isso, para qualquer tarefa envolvendo Bevy:
- Prefira mudanças pequenas e incrementais a refactors grandes de uma vez.
- Se não tiver certeza de uma API (assinatura de sistema, trait bound, nome de método), diga isso explicitamente em vez de inventar uma assinatura plausível.
- Rode `cargo check -p <crate>` mentalmente antes de declarar uma tarefa concluída, mas não confie nisso como substituto de compilação real — o Jay tem o RustRover rodando `cargo check` ao vivo e vai pegar erro de compilação rápido. Se ele apontar um erro, não é sinal de que "quase funcionou", é sinal de reavaliar a abordagem.
- Este projeto pede oversight moderado a alto em qualquer tarefa que toque Bevy. Não assuma que uma tarefa "parecida com a anterior" vai funcionar igual — Bevy já quebrou isso antes.

## Mapa dos crates

- `eigc_app` — binário principal, monta os plugins e o `App` do Bevy.
- `eigc_moons` — perfis de lua (`MoonProfile`), carregamento via RON/`AssetServer`.
- `eigc_scene` — câmera (free-fly, camera lock sol/Júpiter), céu (sol, Júpiter, starfield, eclipse).
- `eigc_terrain` — geração procedural de terreno, kit-of-parts por lua.
- `eigc_common` — código compartilhado entre crates (ex.: matemática comum).
- `eigc_sim` — lógica de simulação (ex.: `TimeFlowPlugin`).
- `eigc_perf` — ferramental de debug/performance, tudo atrás da feature `dev` (fps overlay, diagnostics). Nunca deve vazar pra build de release.
- `eigc_testkit` — utilitários de teste compartilhados.

Regra de dependência: crates de domínio (`eigc_moons`, `eigc_terrain`) não dependem de `eigc_scene`; é `eigc_scene` que depende deles. Se uma mudança exigir o contrário, pare e pergunte antes de prosseguir — provavelmente é sinal de que a lógica está no crate errado.

## Build e teste — sempre escopado por crate

Este é um workspace virtual (sem `[package]` na raiz). Rodar `cargo test` ou `cargo build` sem `-p` compila o workspace inteiro, incluindo `eigc_app`, que puxa o stack de render completo do Bevy. Isso já causou recompilação lenta e não intencional no passado.

```bash
cargo test -p eigc_moons
cargo build -p eigc_app --features dev
```

Use os aliases já configurados em `.cargo/config.toml` quando existirem, em vez de escrever o comando cru.

## Convenções de código (resumo — ver `CONTRIBUTING.md` para o texto completo)

- Sem `use bevy::prelude::*` nem outro wildcard import — todo import explícito.
- Doc comment (`///`, `//!`) obrigatório em todo item público, escrito em português.
- Identificadores em inglês.
- Sem `.unwrap()` fora de teste.
- Comentário inline só para decisão genuinamente não óbvia — não narrar o que o código já deixa claro.
- Testes: unit test inline (`#[cfg(test)] mod tests`) é a única forma de acessar item privado — use para isso. Integração em `crate/tests/` é só para API pública.
- Nome de arquivo de teste reflete comportamento observável, não o nome da função (`camera_lock_orientation.rs`, não `lock_aim_update.rs`).
- Débito técnico consciente vai para `BACKLOG.md` com justificativa — não fica implícito nem é resolvido às pressas só para "limpar".
- Não tornar item `pub` só para facilitar teste. Visibilidade é superfície de API, não conveniência.

## Bugs já caçados — não reintroduzir

- **Duplicação de estado é o bug mais recorrente do projeto.** Sempre que duas fontes de verdade existirem para a mesma grandeza física (ex.: direção do sol em `SkySettings` estático vs. `SkyState` animado por frame), uma delas vai dessincronizar. Antes de adicionar um novo campo de estado, pergunte se ele já existe em outro lugar.
- **`run_if(resource_exists::<T>)` é obrigatório** em qualquer sistema que consome um `Resource` opcional. Faltar isso já causou panic em runtime (`animate_sky_physical`).
- **Guard de `State` é obrigatório antes de qualquer `spawn` em `Update`.** Um sistema de transição sem checar o estado atual já spawnou `DirectionalLight` sem limite e causou OOM.
- **Extraia lógica de sistema para função pura sempre que possível** (ex.: `clamp_pitch`, `resolve_lock_direction`). Função pura testa sem infraestrutura de ECS; sistema que só chama a função pura fica fino e mais fácil de revisar.
- **Caminho de asset é relativo ao manifest dir do crate binário, não à raiz do workspace.** Configuração correta: `AssetPlugin { file_path: "../../assets".to_string(), ..default() }`.

## Convenção de branch

`{tipo}/{label}/nome-da-branch` — exemplos: `feature/melhoria/claude-code-setup`, `bugfix/terreno/nome-da-branch`, `feature/europa/nome-da-branch`. Se uma tarefa não se encaixar em nenhum tipo/label já visto, pergunte ao desenvolvedor humano em vez de inventar um novo.

## Ao final de uma tarefa

- Rode todos os testes para verificar se algum quebrou. (`cargo test`)
- Se descobriu um bug que não vai corrigir agora, registre em `BACKLOG.md` seguindo o formato existente (justificativa + trade-off), não deixe implícito.
- Não marque uma tarefa Bevy como concluída só porque "parece certo" — sinalize incerteza explicitamente se não conseguiu confirmar a API.