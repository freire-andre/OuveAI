# Catálogo de modelos (hash fixado · L14)

> Calculado em 30/10/2026 a partir do download direto dos releases do sherpa-onnx (que não publicam hash).
> Vai virar a fonte do `core::models` na Fase 2.

| id | URL | Tamanho (bytes) | SHA-256 | Extraído |
|---|---|---:|---|---|
| `parakeet-tdt-0.6b-v3-int8` | https://github.com/k2-fsa/sherpa-onnx/releases/download/asr-models/sherpa-onnx-nemo-parakeet-tdt-0.6b-v3-int8.tar.bz2 | 487170055 | `5793d0fd397c5778d2cf2126994d58e9d56b1be7c04d13c7a15bb1b4eafb16bf` | ~640 MB: `encoder.int8.onnx` (622 MB), `decoder.int8.onnx` (11,3 MB), `joiner.int8.onnx` (6,1 MB), `tokens.txt`, `test_wavs/` (de, en, es, fr) |
| `silero-vad` | https://github.com/k2-fsa/sherpa-onnx/releases/download/asr-models/silero_vad.onnx | 643854 | `9e2449e1087496d8d4caba907f23e0bd3f78d91fa552479bb9c23ac09cbb1fd6` | arquivo único |

Licenças: Parakeet TDT v3 CC-BY-4.0 (atribuição obrigatória, L12) · Silero VAD MIT.

## Espelho alternativo (S4, 31/10/2026)

Hugging Face `csukuangfj/sherpa-onnx-nemo-parakeet-tdt-0.6b-v3-int8`, fixado no commit `2bda32ec70b097a55adaa07d9a7173915b43cc78`. Os arquivos vêm soltos, sem o `.tar.bz2`. O SHA-256 bate com os arquivos extraídos do release:

| arquivo | URL (`https://huggingface.co/csukuangfj/sherpa-onnx-nemo-parakeet-tdt-0.6b-v3-int8/resolve/2bda32e…/<arquivo>`) | bytes | SHA-256 |
|---|---|---:|---|
| `encoder.int8.onnx` | `…/encoder.int8.onnx` | 652184281 | `acfc2b4456377e15d04f0243af540b7fe7c992f8d898d751cf134c3a55fd2247` |
| `decoder.int8.onnx` | `…/decoder.int8.onnx` | 11845275 | `179e50c43d1a9de79c8a24149a2f9bac6eb5981823f2a2ed88d655b24248db4e` |
| `joiner.int8.onnx` | `…/joiner.int8.onnx` | 6355277 | `3164c13fc2821009440d20fcb5fdc78bff28b4db2f8d0f0b329101719c0948b3` |
| `tokens.txt` | `…/tokens.txt` | 93939 | git blob `fc43e1c723e262df60b70e1919614417162d1fe2` (calcular o SHA-256 na Fase 2) |

- O cabeçalho `X-Linked-ETag` do HF traz o SHA-256 dos arquivos grandes, mas o catálogo continua com o hash fixado no app.
- O `silero_vad.onnx` do HF (`csukuangfj/vad`, 1,8 MB) é **outra versão**, então não serve de espelho para o Silero. Como ele tem 643 KB, pode ir embutido no instalador (a decidir na Fase 2).
- Dentro do exe, os arquivos soltos são melhores que o `.tar.bz2`: dispensam a extração e permitem retomar e verificar cada arquivo separado.

## Parakeet pt-BR (0.9.2, ADR-009)

A partir da 0.9.2, o modelo de fala é o ajuste pt-BR do Parakeet: o checkpoint `.nemo` de `alexandreacff/parakeet-tdt-v3-ptBR-30s` (commit `3f46ff2c51304da75f4e8754d4b3731ea70cfd9a`, CC-BY-4.0), convertido para o sherpa-onnx int8 pelo script oficial `export_onnx.py` por `joaorura/parakeet-tdt-0.6b-v3-ptBR-sherpa-onnx-int8`, fixado no commit `bf31b612b9ea9602c21b9faf0dec038ff2c9a3fc`, que é o espelho. O SHA-256 foi conferido no arquivo baixado e bate com o README do repositório.

Fonte principal: cópia sem modificação no release do próprio projeto, https://github.com/freire-andre/OuveAI/releases/download/asr-models/parakeet-tdt-0.6b-v3-ptbr-int8.tar.bz2, com 486200022 bytes e SHA-256 `377dccca7fe980eff69108761dc42ab09cd90cf08ce325f6a6f4745f0502faea`. A raiz é `parakeet-tdt-0.6b-v3-ptbr-int8/`, com os 4 arquivos e um `README.txt` de créditos (CC-BY-4.0), que é ignorado na extração.

| id | arquivo | bytes | SHA-256 |
|---|---|---:|---|
| `parakeet-tdt-0.6b-v3-ptbr-int8` | `encoder.int8.onnx` | 652282302 | `260aff7bf778b016b42385e186d88a57224bd6958001993dbb6070a9a75b6690` |
| | `decoder.int8.onnx` | 11845274 | `6b77e877b1dc29ebead07c0f461a7385d1ac5f8eff5be6422fdddb390d4a9967` |
| | `joiner.int8.onnx` | 6355277 | `eb365a33945103cb787aa849b344cbf2abc093b43211e3b969ff97be3fdfc0e4` |
| | `tokens.txt` | 93939 | `d58544679ea4bc6ac563d1f545eb7d474bd6cfa467f0a6e2c1dc1c7d37e3c35d` (igual ao do multilíngue) |

O `parakeet-tdt-0.6b-v3-int8` (multilíngue) continua no catálogo para quem atualizou da 0.9.1: é usado enquanto o pt-BR não estiver instalado e só aparece na tela Modelos se estiver no disco.

## Whisper e FFmpeg no release do próprio projeto (0.9.4, ADR-010)

Cópias sem modificação (mesmo SHA-256 da origem, conferido em 02/11/2026). A fonte própria é a principal, e a origem fica como reserva.

| componente | fonte principal | reserva | bytes | SHA-256 |
|---|---|---|---:|---|
| `whisper-large-v3-turbo-int8` | https://github.com/freire-andre/OuveAI/releases/download/asr-models/sherpa-onnx-whisper-turbo.tar.bz2 | Hugging Face `csukuangfj/sherpa-onnx-whisper-turbo@2ca6ff69fc878651b770880507669577ac41c2ff` (arquivos soltos, "Usar o espelho") | 563790207 | `b11acbbcd660b44a8e0df33724feb5aaa709cf65668f2823d59f656312544f22` |
| FFmpeg 9.0.2 essentials (gyan.dev) | https://github.com/freire-andre/OuveAI/releases/download/ffmpeg-9.0.2/ffmpeg-9.0.2-essentials_build.zip | https://github.com/GyanD/codexffmpeg/releases/download/9.0.2/ffmpeg-9.0.2-essentials_build.zip (automática) | 114768076 | `60f467265b1e312373dbcd92200c2618a74850f98d3d078e94296bb3fa2047ba` |

O release `ffmpeg-9.0.2` também traz o código-fonte (`ffmpeg-9.0.2.tar.xz` do ffmpeg.org, SHA-256 `8c3850283eb25fa026482078a04051e0be17347b09ef81a0849bec15a96e002e`), como a GPL-3.0 exige.
