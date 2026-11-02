# S9 — Parakeet ajustado para pt-BR

> 02/11/2026 · `crates/ouveai-core/examples/s9_parakeet.rs` (sherpa-onnx 2.0.1, mesmos pedaços do app) e onnx-asr 0.12 para as variantes que só existem nesse formato · CPU, 4 threads

## Resultado: **trocar o modelo de fala pelo ajuste pt-BR "30s"**, mantendo os pedaços de até 60 s (ADR-009)

O spike testou as sugestões de um levantamento sobre como melhorar o Parakeet TDT 0.6b v3 em pt-BR: usar um checkpoint já ajustado para pt-BR, trechos de ~30 s, normalizar o volume, fp32 no lugar de int8 e fine-tuning próprio.

## Variantes

| Nome | Origem | Formato |
|---|---|---|
| Base int8 (0.9.1) | `nvidia/parakeet-tdt-0.6b-v3`, pacote do sherpa-onnx | sherpa-onnx |
| Base fp32 | `istupakov/parakeet-tdt-0.6b-v3-onnx` | onnx-asr |
| pt-BR "plus" int8 | fine-tune TAGARELA, export `calneymgp/parakeet-tdt-0.6b-v3-ptBR-TAGARELA-onnx-int8` | onnx-asr |
| **pt-BR "30s" int8** | fine-tune `alexandreacff/parakeet-tdt-v3-ptBR-30s` (TAGARELA), export `joaorura/parakeet-tdt-0.6b-v3-ptBR-sherpa-onnx-int8` | sherpa-onnx |

Licença CC-BY-4.0 em toda a cadeia. O "30s" usa o mesmo `tokens.txt` do base e o export oficial do sherpa-onnx, então entra no app sem mudança de código.

## Áudios curtos (referência montada à mão, sem revisão humana)

Três áudios, 586 palavras: um vídeo narrado (79 s), uma conversa a duas vozes com fala sobreposta (35 s) e o `ptbr_01` (124 s, reunião simulada com nomes próprios e termos em inglês). WER depois de normalizar caixa, acento, pontuação, formas coloquiais e números.

| Configuração | Narrado | Duas vozes | Reunião | Erros | Palavras em inglês |
|---|---:|---:|---:|---:|---:|
| Base int8, até 60 s (0.9.1) | 2,2% | 12,6% | 12,4% | 50 | 4 |
| Base int8, até 30 s | 3,1% | 11,8% | 5,0% | 33 | 0 |
| Base int8 + loudnorm | 2,2% | 9,2% | 9,5% | 39 | 0 |
| Base fp32, até 60 s | 1,3% | 16,0% | 74,8% | 203 | 24 |
| Base fp32, até 30 s | 1,8% | 13,4% | 62,8% | 171 | 23 |
| pt-BR "plus", até 60 s | 1,8% | 25,2% | 4,5% | 45 | 0 |
| pt-BR "plus", até 30 s | 1,8% | 22,7% | 5,8% | 45 | 0 |
| pt-BR "30s", até 60 s | 1,3% | 13,4% | 4,5% | 30 | 0 |
| **pt-BR "30s", até 30 s** | 1,8% | 10,1% | 4,1% | **26** | **0** |
| pt-BR "30s" + loudnorm | 1,3% | 13,4% | 4,1% | 29 | 0 |

Na conversa a duas vozes, uma palavra vale 0,8%: diferenças de 2 ou 3 pontos ali são ruído.

## Podcast (65 min, legenda de referência)

O mesmo podcast da Fase 3 (áudio limpo, uma voz principal, legenda de referência com 14.372 palavras depois da normalização), alinhado em janelas de 300 s. Os números absolutos não batem com os da Fase 3 porque a normalização mudou (números por extenso, formas coloquiais); a comparação vale entre as linhas desta tabela.

| Configuração | WER | Trocas | Faltas | Sobras | Trechos pulados (≥ 4 palavras) |
|---|---:|---:|---:|---:|---:|
| Base int8, até 60 s (0.9.1) | 13,1% | 756 | 904 | 216 | 20 (105) |
| Base int8, até 30 s | 12,8% | 744 | 889 | 208 | 22 (116) |
| **pt-BR "30s", até 60 s** | **11,8%** | 627 | 820 | 242 | 18 (84) |
| pt-BR "30s", até 30 s | 11,9% | 638 | 839 | 231 | 18 (84) |

O ajuste pt-BR tira cerca de 190 erros (−10% relativo), quase todos trocas e faltas, e pula menos trechos. Com ele, 30 s e 60 s empatam. Parte do erro restante vem da própria legenda ("né", "tipo", repetições que ela não transcreve).

## O que cada sugestão fez

- **Checkpoint pt-BR.** O "30s" melhorou nos três áudios e acabou com a deriva para o inglês. O "plus" vai bem em fala de uma voz, mas **apaga a segunda voz** quando há sobreposição ("Você quer moralizar o consumo", "Não", "É mais uma hipocrisia" somem). Para reunião, isso é perda de fala: descartado.
- **Trechos de ~30 s.** No base, 30 s ajuda (tira a deriva para o inglês da reunião). No pt-BR, que já não deriva, o ganho some: empate no podcast (11,9% × 11,8%) e diferença de poucas palavras nos áudios curtos. **Decisão: manter até 60 s** (o dobro de cortes não compra qualidade medida). Pedaços de 30 s são ~15% mais rápidos; se a velocidade virar problema no piloto, é só mudar `media::CHUNK_MAX`.
- **Normalizar volume (loudnorm).** Ajuda o base de forma irregular e não muda nada no "30s". Não vale um passo a mais do FFmpeg.
- **fp32.** Pior: na reunião, o base fp32 transcreveu quase tudo em inglês ("We will initiate this reunion…"). Ainda ocupa 2,5 GB, contra 650 MB.
- **Pós-processamento.** O Parakeet já entrega pontuação e maiúsculas; número por extenso × dígito já é tratado na comparação (`core::compare`).
- **Fine-tuning próprio.** Não testável aqui: exige GPU com NeMo e de 5 a 20+ horas de áudio com transcrição revisada. Fica para depois do piloto, se os erros recorrentes justificarem.

## Velocidade

Igual entre o base int8 e o "30s" (mesma arquitetura e mesma quantização): 16× a 19× o tempo real nos áudios curtos; no podcast, com a máquina livre, 14,2× (base, 60 s), 16,2× (pt-BR, 60 s) e 18,4–18,9× (30 s). Uma segunda rodada do pt-BR com a máquina ocupada deu 9× e 3,6×, descartada. Pedaços de 30 s são um pouco mais rápidos que 60 s, porque o custo do encoder cresce mais que linearmente com o tamanho.

## Instalação pelo app

`ouveai-cli models download parakeet-tdt-0.6b-v3-ptbr-int8` funcionou pelas duas fontes: o pacote do release `asr-models` (conferido, extraído, com o `README.txt` ignorado) e os arquivos soltos do Hugging Face (commit fixado). Nos dois casos, o SHA-256 foi conferido e o modelo foi instalado na pasta de modelos; `ouveai-cli transcribe` passou a usar o modelo pt-BR. Quem atualizar da 0.9.1 continua com o multilíngue até baixar o novo (`models::speech_model`).

## Pendências

- O "30s" não tem avaliação publicada pelo autor. O hash só garante que o arquivo é o que o repositório publicou.
- ~~O download depende de um repositório de usuário no Hugging Face.~~ Resolvido: há uma cópia no release `asr-models` do OuveAI, que é a fonte principal, e o Hugging Face ficou como espelho (ADR-009).
