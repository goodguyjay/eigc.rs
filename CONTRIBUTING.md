# Guidelines de código - eigc-rs

Este documento define como código Rust é escrito, documentado e testado neste projeto.

## 1. Documentação

- Todo arquivo começa com `//!` no topo, resumindo **o que aquele módulo é**, não como ele
  funciona por dentro.
- Todo item público (`struct`, `enum`, campo, função, `impl` público) leva `///` dizendo o que
  ele é. Mesmo critério: o quê, não como.
- Comentários inline (`//`) são exceções, não regra. Só existem quando algo é genuinamente não
  óbvio: motivo de uma escolha não intuitiva, edge case, débito técnico consciente. Um
  comentário que só reformula o nome da variável ou função é ruído e deve ser removido.
- Documentação e comentários em português. Identificadores de código (tipos, funções,
  variáveis, módulos) em inglês.

**Referência de padrão ruim:** arquivo que só implementa um trait sem nenhum `//!`/`///`,
mesmo que o código em si seja trivial. Trivialidade não isenta de documentação de módulo.

## 2. Nomenclatura

- Nomes de variáveis e funções explícitos. Sem abreviação de letra única.
- Exceção aceita: convenções matemáticas estabelecidas (`x`, `y`, `z`, `t` para parâmetro de
  curva, etc.) onde abreviar é o padrão do domínio.

## 3. Imports

- Proibido wildcard import (`use bevy::prelude::*;`, `use crate::foo::*;`), mesmo quando o
  módulo de origem é enorme (ex: `bevy::prelude`). Sempre explícito, item por item.

## 4. Formatação e tipagem

- `rustfmt` obrigatório antes de qualquer commit.
- Tipagem explícita só quando o compilador exige (parâmetro de função, campo de struct,
  ambiguidade de inferência).

## 5. Testes

- Todo commit que introduz ou modifica uma função com lógica não trivial precisa vir com teste
  unitário. Getter/setter puro e código totalmente delegado (ex: `build()` de um `Plugin` que só
  chama `init_asset`) não exige teste próprio.
- **Teste unitário de item privado** (algo sem `pub`, como uma função de recipe interna) vive em
  `#[cfg(test)] mod tests` dentro do próprio arquivo.
- **Teste de integração** (comportamento de um crate inteiro através da API pública, ex: rodar
  `MoonPlugin` dentro de uma `App` de bevy e verificar o estado resultante) vive em `/tests/` na
  raiz do crate/binary correspondente, um arquivo por área de comportamento.
- `rstest` para cobertura paramétrica (ex: mesmo teste rodando para as quatro variantes de
  `MoonId`). `proptest` para invariantes de geração procedural.

## 6. Débito técnico

- Débito técnico é aceitável quando é um trade-off consciente e localizado (ex: acoplamento de
  `TerrainPlugin` a `Handle<MoonProfile>`), não quando é um buraco estrutural que vai exigir
  reescrita de várias partes do sistema para resolver depois.
- Todo débito técnico aceito precisa de uma linha em `BACKLOG.md`, descrevendo o quê e por que
  foi aceito, não só "TODO: arrumar isso depois" sem contexto.
