# Spike S8 · Nemotron 3.5 ASR Streaming 0.6b como revisor (ADR-008)

> 02/11/2026 · spike descartado (o exemplo `s8_nemotron.rs` não ficou no repositório) · build release, mesmo crate `sherpa-onnx` 2.0.1 do app

## Resultado: **Go técnico, No-Go como revisor** (decisão final: Nemotron descartado, ver ADR-008)

O Nemotron carrega e roda no runtime atual, sem trocar de crate. Ele é ~3,5× mais rápido que o Whisper, usa menos da metade da RAM e não entra em laço.

A primeira rodada parecia favorecer o Nemotron, mas ela cortava janelas fixas de 20 s, e isso prejudicava o Whisper. Com as janelas cortadas no silêncio medido, como o app faz, e com os ajustes de comparação abaixo, o resultado se inverteu: **o Whisper acha praticamente os mesmos erros do Parakeet com muito menos sugestões erradas.**

Decisão proposta: o Whisper continua como revisor padrão, e o Nemotron fica como opção experimental, mais rápida.

## Artefato

| Item | Valor |
|---|---|
| Fonte | release `asr-models` do k2-fsa/sherpa-onnx (reexportado em 08/07/2026) |
| Pacote escolhido | `sherpa-onnx-nemotron-3.5-asr-streaming-0.6b-1120ms-int8-2026-06-11.tar.bz2` |
| Tamanho do pacote | 475.276.334 bytes |
| SHA-256 do pacote | `adbdd5e9fef87300c37cebfcfc4f1ebe56845c860c8a760af0a1dd65ce9beed3` |
| Extraído (4 arquivos usados) | 682.215.474 bytes (~651 MB), contra 989 MB do Whisper |
| Espelho no Hugging Face | não encontrado com o nome do pacote; só o GitHub por enquanto |
| Licença | **OpenMDW-1.1**: uso livre, inclusive comercial; quem redistribui mantém o texto da licença e os avisos de origem |

| Arquivo | Bytes | SHA-256 |
|---|---|---|
| `encoder.int8.onnx` | 657601521 | `2fff2166acaa535bd969fb223c1f0783d71029f143cb298bc54c2afe85abf772` |
| `decoder.int8.onnx` | 14978075 | `19f9c98fc6d0a2c33a65a43b36fdb2e914c26c0aa9764be3aebc502a1e982fb0` |
| `joiner.int8.onnx` | 9504438 | `4101c7c679a0bc30483794b27a059e34e79232aa2068d78d51231a22c8b0d7ce` |
| `tokens.txt` | 131440 | `729cc103155bafa785f9cd45746cd41cabe97eab7182fc04d594129587958f8a` |

O pacote também traz `README.md` e `test_wavs/`, que não são usados.

## API real (regra 5)

- **Reconhecedor:** `OnlineRecognizer`, não o `OfflineRecognizer` do Parakeet. A configuração é `model_config.transducer {encoder, decoder, joiner}`, `tokens`, `num_threads`, `provider = "cpu"` e `greedy_search`. O tipo é detectado sozinho, sem `model_type`. Os endpoints ficam desligados.
- **Execução:** alimentar o áudio em pedaços (`accept_waveform`) e decodificar enquanto `is_ready`. No fim, acrescentar 1,5 s de silêncio, chamar `input_finished` e esvaziar a fila.
- **Idioma:** `OnlineStream::set_option("language", …)`, por stream. O mapa do sherpa-onnx dá `pt-BR` → 12 e `pt`/`pt-PT` → 13. Uma string desconhecida cai em `auto`.
- **Resultado:** `text`, `tokens`, `timestamps` (início de cada token) e `segment`/`start_time`/`is_final`. **Não há `durations` nem confiança.** Os tokens usam espaço inicial como marcador de palavra, como no Parakeet.

## Medições

Ryzen 5 5600X, 4 threads, greedy.

| Medida | Nemotron (1120 ms) | Parakeet | Whisper turbo |
|---|---|---|---|
| Carga | 1,4–2,9 s | ~2 s | — |
| Velocidade em janelas de 20 s | 6,5–9,8× tempo real | 17–21× | 2,4–3,0× |
| RAM de pico do modelo | **~780 MB** (762 MB após a carga) | ~700 MB | ~1,8 GB |
| Laços (frase repetida) | nenhum | — | sim, em 1 de 17 janelas |

O áudio de 124 s num só stream levou 12,9 s (9,6×), com RAM estável: o streaming não cresce com a duração. O pico de 1,2 GB medido no áudio longo vem do WAV de 65 min carregado inteiro na memória, não do modelo.

O pacote de 560 ms rodou a 5,4× e não mostrou ganho claro de qualidade. Fica o de 1120 ms, que também tem o menor WER publicado.

## Qualidade

Material:
- uma gravação própria de 2 min em pt-BR, com nomes de pessoas, marcas e termos técnicos (a mesma do S1), em 6 janelas;
- 11 janelas de 20 s espalhadas por uma conversa longa de 65 min, com fala rápida, gíria e sobreposição.

Não havia reunião com áudio salvo nesta máquina.

**Sem referência revisada por pessoa, o indicador usado foi "2 contra 1":** quando o Nemotron e o Whisper concordam contra o Parakeet, o Parakeet provavelmente errou.

| Conjunto | Divergências P×N | Divergências P×W | N e W concordam contra P | Diferença só de repetição | N sozinho (W igual a P) |
|---|---|---|---|---|---|
| Gravação de 2 min | 21–23 | 16 | 6–7 | 0–1 | 9–13 |
| Conversa longa | 54–60 | 43 | 11–18 | 3–4 | 20–24 |

### Onde o Nemotron ajuda

- **Corrige a troca de idioma do Parakeet**, o ponto mais fraco dele no S1: "reunion" → "reunião", "but" → "mas", "or" → "ou", "and" → "pra". O Whisper pega só parte desses casos.
- **Corrige marcas que o Parakeet ouviu errado:** "Hair Dragon" → "Red Dragon".
- **Recupera frases inteiras:** com o áudio de 2 min num só trecho, o Parakeet omitiu várias frases que o Nemotron transcreveu. Em janelas de 20 s, que é como o app trabalha, o Parakeet já não omite.
- **Usa formas de pt-BR**, como "tá" e "pra", que a comparação já trata como iguais às formas por extenso.

### Onde o Nemotron atrapalha

- **Perde palavras na borda da janela:** no começo, falta contexto ao encoder de streaming; no fim, a palavra cortada no meio se perde. A correção testada foi dar 2 s de contexto de cada lado e manter só as palavras cujo primeiro token começa dentro da janela. Os tempos do Nemotron servem só para esse recorte e nunca são gravados. Isso recuperou o começo das janelas.
- **Transcreve hesitações literalmente** ("de de de", "tão tão", "não não"). O Parakeet as limpa, e isso gera divergência falsa.
- **Erra gíria e nomes raros na fala rápida**, com grafias inventadas e uma palavra em espanhol. O Whisper também erra nesses trechos e ainda entra em laço.

### Rodada final: janelas como o app e ajustes de comparação aplicados

**Configuração:**
- janelas de até 20 s cortadas no silêncio medido pelo FFmpeg, iguais às do app, para os três modelos;
- greedy;
- 2 s de contexto de cada lado só para o Nemotron.

**Material:**
- um vídeo narrado por uma voz (79 s);
- um vídeo com duas pessoas discutindo, com interrupções e falas sobrepostas (34 s).

Cada divergência foi julgada pelo contexto da frase.

| Divergências | Narrado: Nemotron | Narrado: Whisper | Duas vozes: Nemotron | Duas vozes: Whisper |
|---|---|---|---|---|
| Mostradas | 12 | 6 | 4 | 4 |
| Revisor certo | 5 | **5** | 0 | 2 |
| Revisor errado | 5 | **0** | 2 | 0 |
| Mista (corrige uma parte e apaga fala) | 0 | 0 | 1 | 1 |
| Incerta, ou formato/hesitação | 2 | 1 | 1 | 1 |

| Velocidade (tempo real) | Parakeet | Nemotron | Whisper |
|---|---|---|---|
| Narrado | 19× | 7,3× | 2,1× |
| Duas vozes | 20× | 8,0× | 2,3× |

**No vídeo narrado:** o Parakeet errou 6 vezes. O Nemotron acertou 5 sugestões, e só ele achou "comidinha para **bebê**", mas também sugeriu 5 grafias erradas ("sobrevência", "quite", "exça"…). O Whisper também acertou 5 sugestões, entre elas "**Eles** mandaram", e não sugeriu nada errado.

**Com duas vozes:** os dois revisores ficam com a voz principal e **omitem as interrupções** da segunda pessoa, que o Parakeet transcreve. O Nemotron também embaralha palavras no trecho sobreposto ("moralisia", "preseria").

**O que testei e descartei:**
- **Beam search no Nemotron** (`modified_beam_search`, 4 caminhos): mesmo resultado no vídeo narrado, pequenas mudanças no de duas vozes, ~30% mais lento.
- **Margem de contexto:** sem ela, o Nemotron volta a errar nas bordas ("comer" → "com", "dias" → "G"). Fica a margem de 2 s, só para ele.

### Rodada de ajustes do Nemotron

Mesmos dois vídeos e mesmas janelas do app. Cada linha muda uma coisa em relação à base (um stream por janela, 2 s de contexto, greedy).

| Configuração | Narrado: certo / errado / neutro | Duas vozes: certo / misto / incerto / errado | Velocidade |
|---|---|---|---|
| Base | 5 / 5 / 2 | 0 / 1 / 1 / 2 | 7,6–8,3× |
| **Um stream no arquivo inteiro + 1,5 s de silêncio no início** | **5 / 3 / 2** | **1 / 1 / 1 / 2** | **8,3–9,2×** |
| 1,5 s de silêncio no início, um stream por janela | 5 / 6 / 2 | 1 / 1 / 1 / 2 | 6,4–7,0× |
| `blank_penalty` 0,5 | 5 / 6 / 1 | 1 / 0 / 2 / 3 | 7,0–8,9× |
| `blank_penalty` 1,0 | 6 / 4 / 0 | 1 / 1 / 1 / 4 | 7,9–9,0× |
| Hotwords (beam de 4, peso 1,5 e 2,0) | 5 / 4 / 2 (nenhum efeito nas palavras-alvo) | 1 / 1 / 1 / 1 | 5,4–6,0× |

**Um stream no arquivo inteiro venceu.**
- O texto de cada janela sai depois, pelo tempo de início dos tokens.
- Sumiram o "Por" apagado no início do arquivo, os "moralisia", "quite" e "demandaram", e o "G" na fronteira das janelas.
- Também é mais rápido, porque o modelo é carregado e aquecido uma vez só.

**As hotwords não funcionam com este pacote.**
- Não há `bpe.vocab` no pacote; testei um gerado a partir do `tokens.txt`.
- Nem as palavras da lista mudaram: "sobrevência", "aguentam" e "exça" continuaram.
- A lista testada eram os próprios erros do teste, então mesmo um efeito positivo não teria valor como evidência.

### Confiança por token: existe, fora do crate

O JSON da API em C (`SherpaOnnxGetOnlineStreamResultAsJson`) traz `ys_probs`, o log da probabilidade de cada token, inclusive em greedy. O crate Rust 2.0.1 descarta esse campo ao desserializar. Isso também vale para `lm_probs`, `context_scores` e `words`, mas esses vêm vazios. No spike, li o JSON pelo ponteiro nativo; no produto, é preciso uma correção no crate ou chamar a API em C diretamente.

Confiança da palavra = menor probabilidade entre os tokens dela. Na configuração vencedora:

| Sugestão do Nemotron | Confiança | Avaliação |
|---|---|---|
| "pressão" (premissa?) | 0,32 | errada |
| "viciar" | 0,34 | incerta |
| "bebê" | 0,51 | certa |
| "para" (da) | 0,61 | errada |
| "cobertor" | 0,66 | certa |
| "sobrevência", "aguentam" | 0,79 | erradas |
| "como", "consumo", "a sua", "exça", "fruta", "dependente" | 0,83–0,95 | certas (e "exça" errada) |

Um corte em 0,4 esconde só lixo e não esconde nenhum acerto. Mas as grafias erradas plausíveis ("sobrevência", "aguentam", "exça") vêm com confiança alta: **a confiança tira o lixo, não o erro de grafia.**

### Idioma: `pt-BR` sai sem pontuação

| `language` | Pontuação e maiúsculas | Observação |
|---|---|---|
| `pt-BR` | **não** (só algumas maiúsculas) | formas de pt-BR |
| `pt`, `pt-PT` | sim | `pt` é pt-PT no mapa do sherpa-onnx |
| `auto` | sim | "tá aqui" virou "daqui" uma vez |

Para achar divergência isso não importa, porque a comparação ignora pontuação e maiúsculas. Importa ao **aplicar**: o texto do revisor substitui palavras do Parakeet que tinham vírgula ou ponto.

## Ajustes aplicados no `core::compare` (valem para qualquer revisor)

1. **Números:** "3" e "três", ou "72" e "setenta e duas", contam como iguais (ADR-008 §5.3), inclusive com "mil" ("dois mil e quinhentos", "mil novecentos e noventa e nove").
2. **Repetição imediata** ("de de de" × "de") não é mostrada.
3. **Frase inteira que só o Parakeet ouviu não vira "apagar".** Quase sempre é uma segunda voz falando por cima, e os revisores omitem a segunda voz.
4. **Ao aplicar,** se o revisor não pontua ou não usa maiúscula, ficam a pontuação final e a maiúscula inicial do Parakeet. Só o texto muda, nunca os tempos.

Com esses ajustes, no vídeo narrado as divergências do Nemotron caíram de 18 para 12 e as do Whisper de 11 para 6. O corte das janelas no silêncio também contribuiu.

**Se o Nemotron entrar como opção experimental:**
- `language = "pt-BR"`, pacote de 1120 ms, greedy;
- **um stream no arquivo inteiro**, com 1,5 s de silêncio no início, e o texto separado nas janelas do app pelo tempo de início dos tokens;
- sugestões com confiança abaixo de 0,4 escondidas, se o crate passar a expor `ys_probs`;
- cancelamento a cada pedaço de 0,5 s.

## Pendências para fechar o S8

- [ ] Transcrição de referência revisada por pessoa em ≥10 trechos, para o WER exigido no ADR-008 §8.2 e §8.3.
- [ ] Trechos de reunião real (Teams, Meet ou Zoom) com áudio salvo.
- [ ] Sandbox com 8 GB, junto da verificação da implementação.

Saídas brutas: JSON por execução, fora do repositório.
