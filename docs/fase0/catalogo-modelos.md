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
