# Um só leitor da GPU (NVML) para o painel de medições

Data: 2026-11-03 · Repositório: `cosmic-taskbar`

## Problema (medido)

`nvmlInit` custa **+20,7 MB e 1 thread** por processo, e não devolve nada no `nvmlShutdown`
(medido com um processo vazio). Hoje o NVML é carregado em **4 processos**: os 3 applets
`central --monitor` (um por monitor, `sample::nvidia` a cada Tick de 5 s) e o `central --panels`
(na primeira abertura do painel de medições). São ~83 MB dos 292 MB de tudo o que o projeto roda.

## Decisão

O hospedeiro dos painéis (`central --panels`, unidade com watchdog, sempre de pé) passa a ser o
**único processo que lê a GPU**. Ele publica a última leitura num arquivo pequeno em
`$XDG_RUNTIME_DIR/cosmic-central/gpu.json` (tmpfs: sem disco), trocado por inteiro (escreve num
temporário e renomeia). Os applets `--monitor` leem o arquivo no lugar do NVML.

- **Cadência:** o hospedeiro lê a GPU a cada 5 s (a dos applets hoje). Com o painel de medições
  aberto ele já lê a 1 s com `full` (VRAM e ventoinha), e publica essas também.
- **Energia da GPU:** o hospedeiro lê o contador (`nvmlDeviceGetTotalEnergyConsumption`) a cada
  60 s, como o applet que conta faz hoje, e o publica com a hora da leitura. O applet que conta
  (o dono do `energy.lock`) usa esse par no lugar de ler o NVML; a conta da energia não muda.
- **Sem hospedeiro, nada se perde:** um applet que não acha o arquivo, ou o acha com mais de 15 s,
  lê o NVML ele mesmo, como hoje (o custo de memória volta só nesse caso).
- **AMD** (sysfs) continua lida em cada applet, como hoje: não tem o custo do NVML.
- O resto da leitura (CPU, memória, discos, rede, sensores, RAPL) continua em cada applet: é /proc e
  /sys, barato.

## O que não muda

Os números, a frequência na barra (5 s) e no painel (1 s), o histórico, a energia em Wh, o
isolamento (cada parte no seu processo; o hospedeiro travado é reiniciado pelo watchdog e os
applets caem no NVML próprio).

## Custo aceito

A leitura da GPU na barra pode estar até um intervalo (5 s) mais velha, porque o relógio do
hospedeiro e o do applet não batem juntos.

## Ganho esperado

~62 MB de PSS (292 → ~230 MB), 3 threads e dois terços das chamadas ao NVML a menos.
