# ADR-001 · Stack e ASR

- **Status:** aceito (Fase 0, spikes S1 e S5) · 30/10/2026
- **Relatórios:** [`docs/fase0/s1-asr.md`](../fase0/s1-asr.md) · [`docs/fase0/s5-baseline.md`](../fase0/s5-baseline.md)

## Decisão

| Camada | Escolha | Versão fixada na Fase 0 |
|---|---|---|
| Shell | Tauri v2 | 2.12.5 |
| ASR | crate oficial `sherpa-onnx` (wrapper seguro sobre a C API), link estático | 2.0.1 |
| Modelo | Parakeet TDT 0.6b v3 int8 (`model_type = "nemo_transducer"`) | hash em `docs/fase0/catalogo-modelos.md` |
| Decodificação | `greedy_search` | — |
| Threads | arquivo: `min(4, núcleos)` · ao vivo: 2 (L9) | — |

## Campos reais (regra 5)

`OfflineRecognizerResult { text, tokens: Vec<String>, timestamps: Option<Vec<f32>>, durations: Option<Vec<f32>> }`. Com o Parakeet v3, os três vetores vêm preenchidos e com o mesmo tamanho, em passos de 80 ms. O marcador de palavra é o **espaço inicial** do token.

## Medido (Ryzen 5 5600X, 4 threads, áudio pt-BR de 124 s)

- Carga do modelo: 3,26 s no host, 3,70 s no Sandbox.
- Fator de tempo real (duração ÷ tempo): **10,6×** no host, 9,8× no Sandbox.
- RAM de pico: 724 MB depois da carga e 1.643 MB transcrevendo 124 s num stream só.

## Consequências

- **Regra 2 ajustada:** o fim da palavra ignora tokens que são só pontuação, porque no Parakeet a pontuação recebe timestamp depois da pausa.
- **Heurística 4 de "candidatos a erro":** palavras curtas de inglês/espanhol. O Parakeet v3 não fixa o idioma e troca, por exemplo, "reunião" por "reunion" e "mas" por "but".
- **Sem hotwords:** com este pacote, o `modified_beam_search` não codifica hotwords (falta o `bpe.vocab`) e ainda omitiu frases. Vai para o backlog.
- **Sem confiança por token no wrapper.** Se a Fase 3 precisar, avaliar o JSON cru via `sherpa-onnx-sys`.
- **Processo único (6.5)** confirmado: o modelo carrega dentro do processo Tauri sem conflito de runtime (ADR-003).
