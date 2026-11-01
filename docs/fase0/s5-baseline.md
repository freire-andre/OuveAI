# Spike S5 · Linha de base de desempenho (Tauri vazio)

> 30/10/2026 · `spikes/s5-tauri/` · Tauri **2.12.5**, release, HTML puro (sem framework), `+crt-static`

## Método

- `S5_WINDOWS` = janelas criadas no `setup` (0 = só o processo, como na bandeja); o processo não sai sem janelas (`prevent_exit`).
- **Abertura** = ms desde a criação do processo (`GetProcessTimes`) até o `PageLoadEvent::Finished` da última janela.
- **RAM** = soma do *Working Set – Private* do app e de todos os processos descendentes (`msedgewebview2`), 6 s depois de pronto.
- 5 rodadas por cenário (script: `s5_measure.ps1`, mantido fora do repositório).

## Resultados (host: Ryzen 5 5600X, 32 GB, NVMe, Win11 26H2, WebView2 156)

| Janelas | Abertura 1ª rodada | Abertura mediana | RAM total (mediana) | RAM só do exe | Processos |
|---|---:|---:|---:|---:|---:|
| 0 | 81 ms | 21 ms | **1,4 MB** | 1,4 MB | 1 |
| 1 | 642 ms | 383 ms | **70,5 MB** | 3,5 MB | 7 |
| 2 | 476 ms | 489 ms | **87,5 MB** | 3,6 MB | 8 |

- A 2ª janela custa só **+17 MB** (compartilha o processo do navegador e o de GPU do WebView2).
- Quase toda a RAM fica no WebView2, não no exe.
- Sem janela, o custo é desprezível → **janelas sob demanda (6.2) confirmadas**: fechar a principal devolve ~70 MB.

**Windows Sandbox** (sem VC++ Redist, 1 janela, 3 rodadas): 533 / 481 ms, 67,4 MB, 7 processos. ✅ abre.

## Tauri + sherpa-onnx no mesmo exe (fecha a pendência do ADR-003)

- Feature `sherpa` do spike: o mesmo exe cria o `OfflineRecognizer` com o Parakeet antes de abrir a janela.
- Link **sem LNK4098**. O `tauri-build` usa o CRT híbrido (runtime C++ estático + UCRT dinâmico, `api-ms-win-crt-*`, que faz parte do Windows 10+), e as libs `/MT` do sherpa convivem com isso.
- Exe: **26,0 MB**, sem `VCRUNTIME*`.
- Host: modelo carregado em 3,07 s e janela pronta em 3,47 s.
- **Sandbox:** `sherpa_ok` em 4,5 s e janela em 5,0 s. ✅

> No app real a carga do modelo vai para uma thread, para não atrasar a janela (L10).

## Calibração da seção 4 (aplicada no plano)

| Métrica | Antes | Agora | Por quê |
|---|---|---|---|
| RAM na bandeja | < 40 MB | **< 30 MB** | Base de 1,4 MB. Sobra para bandeja, SQLite, logs e o áudio ocioso |
| RAM com a janela principal | < 150 MB | **< 120 MB** | Base de 70 MB. Sobra ~50 MB para Svelte, listas e busca |
| Abrir a mini | — | **< 600 ms** | Criar uma janela custa ~400–500 ms. Como a captura começa na hora (L10), esse tempo não perde áudio. Pré-criar a mini oculta custaria ~70 MB parado na bandeja, então só se a Fase 1 mostrar que incomoda |
| Abertura a frio | < 1 s | mantida | 642 ms na 1ª execução, 383 ms de mediana |
