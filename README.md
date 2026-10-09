# eigc-rs

Simulação interativa das quatro luas galileanas de Júpiter (Europa, IO, Ganimedes e Calisto),
construída em rust + bevy, com terreno procedural calibrado por características geológicas
reais de cada lua e texturas públicas da NASA/USGS.

Esse projeto toma proveito do timing do lançamento da missão Europa Clipper da NASA, 
que foi lançada em 2024 e chegará a Europa em 2030. A missão JUICE da ESA, que também estudará as luas galileanas, 
foi lançada em 2023 e chegará a Júpiter em 2030.

## Rodando o projeto

```bash
cargo run -p eigc_app
```

Isso sobe o menu inicial, com as quatro luas lado a lado. Clique em Europa para ver a ficha e em
"Explorar" para carregar o perfil da lua, gerar o terreno e entrar na simulação. As outras luas
ainda não estão implementadas e mostram apenas um aviso.

- `eigc_common`: matemática pura e constantes universais
- `eigc_sim`: estado de simulação (`TimeFlow`, `SimSet`)
- `eigc_moons`: dados de calibração por lua (`MoonProfile`), carregados como
  asset `.ron` via `AssetServer`, com hot reload, e dados descritivos estáticos de cada lua
  (`MoonInfo`) usados pelo menu
- `eigc_menu`: menu inicial de seleção de lua (cena 3D com os modelos `.glb`, ficha da lua e
  tela de carregamento)
- `eigc_terrain`: geração procedural de terreno, composta por um kit de
  peças genérico (`height/`, `mesh.rs`) e receitas específicas por lua
  (`recipe.rs`)
- `eigc_scene`: câmera de voo livre e céu (sol, Júpiter, estrelas, eclipse e luz refletida de
  Júpiter)
- `eigc_app`: binário principal, amarra os plugins e o estado da aplicação (menu, carregamento
  e simulação)
- `eigc_perf`: instrumentação de performance
- `eigc_testkit`: fixtures de teste compartilhadas entre crates
  (`dev-dependency` apenas)

## Contribuindo

Leia `CONTRIBUTING.md` antes de abrir pull request. Principais pontos: Documentação
obrigatória em todo item público (`//!` `/` `///`, em português), código em inglês,
sem imports "*" e teste para qualquer lógica não trivial.

Pendências conhecidas e débito técnico consciente estão listados em `BACKLOG.md` organizados
por área.

## Stack

Rust + Bevy 0.18.1, `ron`/`serde` para assets, `noise` para geração procedural,
`rstest`/`proptest` para testes.