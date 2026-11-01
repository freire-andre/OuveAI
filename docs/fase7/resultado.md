# Fase 7: comparação com o Whisper (Parte 4)

Data: 01/11/2026 · decisões em [ADR-006](../adr/006-comparacao-whisper.md)

## O que entrou

- **Modelo opcional Whisper large-v3-turbo int8** (989 MB), no catálogo de Modelos com a nota "Opcional". Ele não entra no primeiro uso. O painel de comparação oferece o download quando falta.
- **`core::compare`**
  - O Whisper roda em janelas de 20 s cortadas em silêncio.
  - O alinhamento palavra a palavra usa o crate `similar`, com formas coloquiais e muletas tratadas como iguais.
  - Janelas em que o Whisper entra em laço, inventa ou engole fala são descartadas.
  - O texto escolhido é distribuído nos tempos do Parakeet.
- **CLI:** `ouveai-cli compare <arquivo> [--json saida.json] [--chunk N]` mostra a velocidade dos dois modelos, os trechos descartados e as divergências.
- **Tela "Comparar com o Whisper"** no editor de arquivos e nas reuniões com áudio salvo.
  - Cada trecho divergente aparece lado a lado: Parakeet (atual) | Whisper.
  - A pessoa escolhe um a um ou usa "Whisper em todos" e "Manter todos", e depois clica em "Aplicar N trocas".
  - A tela avisa em quais trechos o Whisper se perdeu.
  - Cancelar mostra "Cancelando…" na hora.
- **Aplicar** troca as palavras numa transação (`Store::replace_words`).
  - Os segmentos tocados ficam marcados como editados, e a busca se atualiza.
  - O app recusa se o texto mudou desde a comparação.
  - Em arquivo, `.json`, `.txt` e `.srt` são regenerados.
- **Recursos:** uma comparação por vez. Ela não começa durante gravação nem com a fila de arquivos rodando, para se uma gravação começar, e a fila espera por ela.
- **Logs:** só contagens e tempos, nunca texto (regra 8).

## Medições

**Tamanho da janela do Whisper** (recorte de 3 min, CPU de 4 threads):

| Janela | Laços | Palavras engolidas | Velocidade |
|---|---|---|---|
| 30 s (limite do modelo) | 3 | 56 | 2,0× tempo real |
| **20 s** (escolhida) | **0** | **15** | 1,9× |
| 15 s | 0 | 52 | 2,0× |

**Ruído da comparação:** na primeira versão, 25% das palavras apareciam como divergentes, quase tudo muleta ("né", "tipo"), forma coloquial ("tá", "pra") ou laço do Whisper. Depois das regras do ADR-006, ficou em cerca de 10%, e o que sobra é diferença de verdade: nomes próprios, termos em inglês e palavras trocadas.

**Velocidade:** o Whisper roda a cerca de 2× o tempo real; o Parakeet, a 13×. Comparar leva cerca de metade da duração do áudio.

**RAM (CLI):** 1,07 GB na fase do Parakeet e 1,8 GB na do Whisper. Os dois não ficam carregados juntos na CLI.

## Verificação

**Testes automáticos:** 59 no core (eram 46). Os novos cobrem o alinhamento, as muletas e formas coloquiais, a detecção de laço, o descarte de janelas, a comparação por segmento, a redistribuição de tempos e `replace_words` (ordem, tempos, marca de editado e busca). O `clippy` não aponta nada no código da fase (só 3 avisos antigos num exemplo de captura de áudio), e o `svelte-check` não tem erros.

**Windows Sandbox** (instalador com a Fase 7, sem rede, 8 GB):

| Passo | Resultado |
|---|---|
| Modelos sem o Whisper | Aparece com a nota "Opcional" |
| Comparar sem o Whisper | Painel "Para comparar, baixe o Whisper (989 MB)" com o cartão de download |
| Reunião com áudio salvo (~1 min, 2 execuções) | Comparada em 20–23 s; 2 e 6 trechos; "Aplicar" trocou o texto |
| RAM na reunião | 1200 MB antes, pico de 2939 MB, 1205 MB depois |
| Arquivo WAV de 2 min 4 s (2 execuções) | Comparado em 50–53 s; 18 trechos; 1 janela descartada por laço |
| RAM no arquivo | 429–433 MB antes, pico de 2167 MB, 452–460 MB depois |
| Aplicar 18 trocas no arquivo | `.srt` ao lado do original regenerado |
| Cancelar no meio | "Cancelando…" em 1 s; o painel some em 11 s, sem erro; a comparação seguinte roda normalmente |
| Log do app | Só contagens e tempos |

A RAM volta ao nível de antes quando a comparação termina: o Whisper é liberado.

## Limites conhecidos

- Uma invenção curta do Whisper, de poucas palavras, ainda pode aparecer como trecho divergente. Quem decide é a pessoa.
- Na reunião, segmentos de mais de 30 s e ecos ficam de fora da comparação.
- Em máquinas de 8 GB, a comparação soma cerca de 1,7 GB enquanto roda.
