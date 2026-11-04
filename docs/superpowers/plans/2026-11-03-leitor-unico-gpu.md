# Um só leitor da GPU: plano

> Spec: `docs/superpowers/specs/2026-11-03-leitor-unico-gpu-design.md`. Ninguém commita sem pedido.
> Base (03/11, repouso 60 s): 26 processos, 292 MB PSS, 153 threads; `central --monitor` 3 × 28,6 MB.

## 1. Publicação da GPU (`src/central/monitor/gpu_share.rs`, novo)

- [x] Testes primeiro: escrever e ler de volta uma leitura; arquivo velho (> 15 s) ou ausente dá
  `None`; arquivo corrompido dá `None` sem pânico; a troca é atômica (temporário + rename).
- [x] `Gpu` ganha `Serialize, Deserialize`; o arquivo leva `Gpu` e a hora da leitura (Unix ms).
  A energia vai dentro do próprio `Gpu` (`energy_mj`): quem publica repete o último contador lido
  quando a leitura não trouxe um (`LAST_ENERGY`), em vez de um par `(mJ, hora)` à parte.
- [x] `publish(&Gpu)` e `latest() -> Option<Gpu>` (idade máxima fixa, `MAX_AGE` = 15 s).

## 2. Hospedeiro publica (`src/central/panels/mod.rs`)

- [x] Uma tarefa no hospedeiro, sempre ativa: a cada 5 s (`Message::ShareGpu`),
  `sample::share_gpu(energy = a cada GPU_ENERGY_EVERY)` em `slow::blocking(Slow::Sample)`, que
  publica.
- [x] Com o painel de medições aberto, a leitura de 1 s do `Monitor` hospedado também publica (e a
  tarefa de 5 s cede a vez).

## 3. Applets leem (`src/central/monitor/mod.rs`, `sample.rs`)

- [x] `Depth` ganha `gpu: GpuSource` (`Shared` ou `Publish`; o plano dizia `Own`): o applet usa
  `Shared` e lê o NVML ele mesmo se `latest()` for `None` ou se a leitura for completa (`full`);
  o hospedeiro usa `Publish`: lê e publica (`pick_gpu`).
- [x] A energia da GPU do applet que conta vem do `energy_mj` publicado.
- [x] Teste: com arquivo fresco o applet não chama o NVML (`pick_gpu` em `sample.rs`, 5 testes).

## 4. Verificar

- [x] `cargo test` verde (953 ok, 5 ignorados); `cargo build --release` ok (03/11).
- [x] Instalado (commit afea2fd); os 3 `--monitor` sem `libnvidia-ml`/`libcuda` no `maps`, PSS
  3,6 / 7,6 / 7,6 MB; só o `--panels` mapeia o NVML (34,5 MB); `gpu.json` com 1 s de idade.
- [ ] (falta o usuário) Na tela, com o usuário: GPU na barra e no painel iguais a antes; energia somando.
- [ ] (não feito: reinicia o hospedeiro) Parar o `central-panels` por 30 s: os applets voltam a ler o NVML sozinhos (o NVML não se descarrega: a memória
  dele fica nesse applet até ele reiniciar); relançado o hospedeiro, eles voltam ao arquivo.
