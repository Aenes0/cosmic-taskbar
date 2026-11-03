# Um só leitor da GPU: plano

> Spec: `docs/superpowers/specs/2026-11-03-leitor-unico-gpu-design.md`. Ninguém commita sem pedido.
> Base (03/11, repouso 60 s): 26 processos, 292 MB PSS, 153 threads; `central --monitor` 3 × 28,6 MB.

## 1. Publicação da GPU (`src/central/monitor/gpu_share.rs`, novo)

- [ ] Testes primeiro: escrever e ler de volta uma leitura; arquivo velho (> 15 s) ou ausente dá
  `None`; arquivo corrompido dá `None` sem pânico; a troca é atômica (temporário + rename).
- [ ] `Gpu` ganha `Serialize, Deserialize`; o arquivo leva `Gpu`, a hora da leitura (Unix ms) e o
  par da energia `(mJ, hora)` quando houver.
- [ ] `publish(&Gpu, energy)` e `latest(max_age) -> Option<Shared>`.

## 2. Hospedeiro publica (`src/central/panels/mod.rs`)

- [ ] Uma tarefa no hospedeiro, sempre ativa: a cada 5 s, `sample::gpu_only(full=false,
  energy=a cada 60 s)` numa thread `slow::blocking` (prioridade idle), e `publish`.
- [ ] Com o painel de medições aberto, a leitura de 1 s do `Monitor` hospedado também publica.

## 3. Applets leem (`src/central/monitor/mod.rs`, `sample.rs`)

- [ ] `Depth` ganha `gpu: GpuSource` (`Shared` ou `Own`); o applet usa `Shared` e cai em `Own` se
  `latest(15 s)` for `None`. O hospedeiro usa `Own`.
- [ ] A energia da GPU do applet que conta vem do par publicado (convertido para `Instant`).
- [ ] Teste: com arquivo fresco o applet não chama o NVML (contador de chamadas em teste).

## 4. Verificar

- [ ] `cargo test` verde; `cargo build --release`.
- [ ] Instalar, recarregar; medir de novo com `~/.cache/taskbar-soak/medir.py 60`: os 3
  `--monitor` sem `libnvidia-ml`/`libcuda` no `maps` e PSS ~8 MB cada.
- [ ] Na tela, com o usuário: GPU na barra e no painel iguais a antes; energia somando.
- [ ] Parar o `central-panels` por 30 s: os applets voltam a ler o NVML sozinhos (o NVML não se descarrega: a memória
  dele fica nesse applet até ele reiniciar); relançado o hospedeiro, eles voltam ao arquivo.
