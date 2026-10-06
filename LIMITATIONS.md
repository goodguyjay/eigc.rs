# LIMITATIONS.md

Limitações conhecidas do ambiente de desenvolvimento. Nenhuma delas é um defeito do código do projeto.

## `cargo test` no workspace inteiro pode falhar por falta de memória

### O que acontece

Rodar o workspace inteiro de uma vez (`cargo test`, `cargo test-all`, `cargo test --workspace`) pode falhar de forma intermitente, com mensagens como:

```
error: linking with `lld-link.exe` failed: exit code: 3
  = note: LLVM ERROR: out of memory
          Allocation failed
error: linking with `lld-link.exe` failed: exit code: 0xc000001d
error[E0463]: can't find crate for `bevy`
memory allocation of 2097152 bytes failed
rustc-LLVM ERROR: out of memory
```

A falha é intermitente: o mesmo comando falha numa execução e passa na seguinte, com o mesmo código.

### Por quê

É falta de memória do sistema (memória comprometida do Windows) durante a compilação paralela, e não um problema do linker nem do código:

- O Cargo usa por padrão um job por thread lógica. Um workspace com Bevy compila vários crates grandes e linka vários executáveis de teste ao mesmo tempo, e cada `rustc` ou `lld-link` pode consumir bastante memória.
- Quando a memória comprometida do sistema chega ao limite, a alocação falha. Isso aconteceu tanto no `lld-link` quanto no próprio `rustc`, o que descarta o linker configurado em `.cargo/config.toml` como causa.
- Os `E0463 can't find crate for bevy` são consequência: o `rustc` de uma dependência morreu sem gerar o `.rlib`, e os crates que dependem dele não o encontram.
- Na máquina em que o problema foi medido, a memória comprometida estava em ~26 GB de um limite de ~33 GB mesmo com o sistema ocioso, deixando só ~6,6 GB de folga. Isso é o que decide se um build paralelo completo cabe ou não.
- O comando do `rustc` que falhava sob o Cargo compilou normalmente (exit 0) quando executado isoladamente, o que confirma que os artefatos em `target/` não estão corrompidos.

### Como afeta

- **Só atrapalha, não bloqueia.** Basta repetir o comando ou usar um dos caminhos abaixo.
- Afeta principalmente os comandos que compilam muita coisa em paralelo: o workspace inteiro, e builds do zero depois de mexer num crate do qual muitos outros dependem (por exemplo `eigc_moons`).
- Os aliases por crate de `.cargo/config.toml` (`cargo test-eigc-moons`, `cargo test-eigc-menu` etc.) compilam bem menos em paralelo e passaram de forma consistente.
- Quando o build falha por falta de memória, o `target/` fica válido. O Cargo só recompila o que faltou na próxima execução.

### Como contornar

Em ordem de custo:

1. Usar os aliases por crate, que é a convenção do projeto.
2. Repetir o comando. Como o Cargo reaproveita o que já compilou, a segunda tentativa costuma passar.
3. Limitar o paralelismo pontualmente, sem mexer em nenhuma config: `cargo test --jobs 4`. Em testes feitos para este registro, `--jobs` 1, 4, 8 e 16 passaram, mas num momento em que havia mais memória livre. Valores menores são mais seguros.
4. Liberar memória do sistema antes de um build completo (fechar navegador e outros aplicativos pesados).
5. Rodar a suíte completa numa máquina com mais RAM. É o que está planejado para a verificação final de cada tarefa.

Não requer atenção no momento.
