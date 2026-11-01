# Fase 3 · Importar arquivo (Partes 2 e 3 do prompt.txt)

> 31/10/2026 · instalador NSIS 8,79 MiB (o sherpa-onnx e o onnxruntime entram estáticos; o exe só depende de DLLs do Windows) · Parakeet TDT 0.6B v3 int8 · `zip` 9.2 · `serde_json` 1.0

## O que existe

**Core (`ouveai-core`)**
- `media`: FFmpeg → WAV temporário 16 kHz mono PCM16 em `tmp\` (apagado no fim, inclusive se der erro ou for cancelado) e `silencedetect` (-35 dB, 0,3 s). O cancelamento mata o processo do FFmpeg.
- **Pedaços de até 60 s**, cortados no meio do maior silêncio entre 45 s e 60 s. Sem silêncio na janela, o corte cai no quadro de 20 ms mais baixo e o usuário é avisado: até 3 cortes, com os tempos; acima disso, um aviso-resumo. O offset de cada pedaço é somado aos tempos. Veja "Tamanho do pedaço" abaixo.
- `asr`: reconhecedor offline do sherpa-onnx 2.0.1 (`nemo_transducer`, `greedy_search`, até 4 threads). Recusa trechos acima de 390 s, porque o encoder aborta o processo acima de 400 s.
- `words` (regra 2): o token com espaço inicial abre a palavra. O fim vem do `durations` do modelo (sem contar a pontuação). Se não houver `durations`, o fim é o início da próxima palavra, recuado ao primeiro silêncio medido entre as duas e limitado a +1,5 s. Nada é interpolado.
- `subs`: legenda com até 36 caracteres por bloco. Quebra numa pausa medida ≥ 0,45 s, no fim de frase, ou na vírgula depois de 20 caracteres. O bloco sai 0,1 s depois da última palavra, dura no mínimo 1 s e respeita 0,08 s até o próximo. `.srt` com CRLF; `.txt` com parágrafo depois de fim de frase seguido de silêncio ≥ 2 s.
- `transcribe`: `.json` (formato do prompt.txt, 3 casas, uma palavra por linha), `.txt` e `.srt` **ao lado do arquivo, com o mesmo nome**. `--from-json` regenera `.txt`/`.srt` depois de corrigir o `.json` à mão.
- `suspects`: candidatos a erro (nome próprio no meio da frase, palavra curta de inglês/espanhol, grafia de Portugal), com a taxa por minuto. Acima de 3/min, recomenda a Parte 4 (Whisper). A confiança por token não existe no sherpa-onnx 2.0.1, então essa heurística ficou sem fonte.
- `store` (migração 2): `sessions` (+ coluna `pauses` em JSON, para refazer a legenda depois de uma correção), `segments` (canal `ARQUIVO`), `words`. `recover_orphans` na inicialização.

**App**
- **Tela Arquivos:** arrastar e soltar ou clicar para escolher (vários de uma vez). Fila com etapa e progresso por pedaço, cancelar, tirar da fila, lista de transcritos. Sem FFmpeg, mostra o cartão da Fase 2 e começa sozinho quando ele fica pronto.
- A fila carrega o modelo uma vez por lote e solta a memória quando esvazia. Durante uma gravação, ela espera: a reunião tem prioridade (L18).
- **Editor:** relatório (carga, transcrição, fator de tempo real, partes, cortes forçados), candidatos destacados com o motivo, clique na palavra para corrigir. **Só o texto muda; `start`/`end` nunca.** Enter salva e regenera o `.json`/`.txt`/`.srt` ao lado do original; Esc cancela. Também tem Exportar `.srt`/`.txt`/`.json`, Abrir pasta e Apagar (os arquivos ao lado do original ficam).
- **CLI:** `ouveai-cli transcribe <arquivo> [--check] [--chunk N]` e `transcribe --from-json <json>`. O `--chunk` serve só para testes de qualidade.
- **Regra 8:** o log guarda duração, tempos e contagens; nunca texto, título ou nome de arquivo (conferido no Sandbox).

## Tamanho do pedaço (decidido com o usuário)

O prompt.txt pede pedaços de 10 min. Isso não funciona aqui por dois motivos:

1. **O encoder do Parakeet em ONNX aborta o processo acima de 400 s** (5000 quadros; `broadcast ... 1000 by 6000` com 480 s). Por isso a primeira decisão foi 5 min.
2. **Em trechos longos com ruído de fundo, o modelo pula frases inteiras.** Mesmo áudio de 2 min (gravação com ruído contínuo, sem nenhum silêncio medido):

| Divisão | Palavras | Observação |
|---|---|---|
| 1 × 124 s | 196 | frases inteiras faltando |
| 90 s + 34 s | 210 | |
| 2 × 62 s | 227 | o texto mais fiel |
| 4 × 31 s | 213 | o fim deriva para o inglês (pouco contexto para a detecção de idioma) |

Validação com o podcast do usuário (65 min, áudio limpo, legenda de referência com 14.360 palavras; script `wer.py` alinhado em janelas de 300 s):

| Pedaço | WER | Trocas | Faltas | Sobras | Trechos pulados (≥ 4 palavras) |
|---|---|---|---|---|---|
| até 5 min | 14,7% | 1017 | 901 | 199 | 15 (67 palavras) |
| até 90 s | 14,3% | 978 | 872 | 200 | 12 (65) |
| **até 60 s** | **14,3%** | 963 | 890 | 194 | 16 (81) |
| até 45 s | 14,2% | 954 | 893 | 186 | 16 (77) |

No áudio limpo, o tamanho quase não importa. No áudio ruidoso, 60 s recupera o que 5 min perde. Parte dos 14% vem da própria referência ("né", "tipo", números escritos de outro jeito). **Decisão: até 60 s, com corte entre 45 e 60 s.** Com a regra final, o áudio de 2 min deu 229 palavras.

## Medições

| Medida | Resultado |
|---|---|
| Carga do modelo | 1,6–2,8 s |
| 124 s, numa passada só (máquina livre) | transcrição em 8,9 s, **13,9×** o tempo real |
| 124 s pelo app no Sandbox (vídeo MP4) | 9,6 s, **12,9×** |
| Podcast de 65 min, tempo total da CLI (carga + conversão + transcrição) | 5 min: 7 min 45 s · 90 s: 5 min 22 s · 60 s: 4 min 44 s · 45 s: ~4 min 45 s |
| 752 s com pedaços de 60 s | 4,1× (**com a CPU em 97% por outro programa**; refazer com a máquina livre) |
| RAM durante a transcrição | ~2 GB com trechos de 220–300 s |

As velocidades do podcast foram medidas em sequência, numa máquina que pode ter estado ocupada. A tendência (pedaços menores são mais rápidos, porque o custo do encoder cresce mais que linearmente) bate com as medidas da máquina livre: 124 s → 13,9×; 300 s → 8,8×; 400 s → 7,3×.

## Testes

| Teste | Resultado |
|---|---|
| `cargo test -p ouveai-core` | 29 passando (silencedetect, cortes, regra 2, legenda, JSON, banco, candidatos, zip) |
| clippy | limpo no código da fase (os 3 avisos são do exemplo do spike S3) |
| svelte-check | 0 erros, 0 avisos |
| **Windows Sandbox** (sem rede; FFmpeg copiado para `bin\`; UI controlada por UI Automation) | instalação ok · Arquivos → escolher o `Reunião de teste.mp4` → transcrito em 13 s · `.json`/`.txt`/`.srt` ao lado do vídeo · editor abre · correção de "reunion," → "reunião," muda **só essa palavra** no `.srt` e no `.json` · Exportar `.srt` igual ao do lado do vídeo · `tmp\` vazio · nome do arquivo **ausente** do log |
| Áudio com nome acentuado (`Reunião de teste.mp4`) | ok |

## Limitações conhecidas

- **Áudio com ruído de fundo contínuo não tem silêncio medido** com -35 dB (piso de ruído ≈ -19 dB na gravação de teste). Aí os cortes caem no ponto de menor volume, e os fins de palavra vêm só do `durations` do modelo. Os tempos continuam medidos, mas a legenda quebra menos por pausa.
- A taxa de candidatos fica alta em fala com muitos nomes próprios: 12–15/min, a maioria nomes reais. É um aviso, não um erro.
- "porque" e "donde" saíram da lista de espanhol, porque também são português.
