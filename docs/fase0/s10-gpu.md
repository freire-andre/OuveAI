# S10 — Transcrição pela placa de vídeo (GPU)

> 02/11/2026 · máquina de desenvolvimento com NVIDIA GeForce RTX 4060 Ti (8 GB), driver 617.42, CUDA Toolkit 12.8 · sherpa-onnx 2.0.1

## Resultado: **No-Go.** Na placa, a transcrição ficou mais lenta que na CPU

A 0.9.3 mantém a transcrição **100% na CPU**, sem opção de placa de vídeo. A detecção da placa fica só no log e no relatório de diagnóstico, para suporte.

## Detecção (entregue)

- `sys::gpus()` lista as placas pelo DXGI (`CreateDXGIFactory1` → `EnumAdapters1` → `GetDesc1`). Conta como dedicada uma placa que não é o adaptador de software e tem 512 MB ou mais de memória própria, o que exclui a fatia reservada das integradas Intel. Sem dependência nova: o crate `windows` já estava no projeto.
- `cuda_driver`: placa NVIDIA com `nvcuda.dll` no System32 (driver instalado).
- Aparece no log de inicialização e no relatório de diagnóstico. Se o DXGI falhar, a lista vem vazia e o app abre normalmente. Nesta máquina: `NVIDIA GeForce RTX 4060 Ti, 7949 MB, driver CUDA: sim`.

## O que o sherpa-onnx oferece para GPU no Windows

| Opção | Situação |
|---|---|
| CUDA (NVIDIA) | Build pronto no release v2.0.1: `…-cuda-12.x-cudnn-9.x-onnxruntime1.28.2-win-x64-cuda.tar.bz2` (570 MB) e a variante CUDA 13. Bibliotecas **dinâmicas** (`onnxruntime.dll`, `onnxruntime_providers_cuda.dll`, `sherpa-onnx-c-api.dll`) |
| DirectML (qualquer placa, inclusive Intel/AMD) | **Não há build pronto.** Exigiria compilar o sherpa-onnx com `SHERPA_ONNX_ENABLE_DIRECTML` |

O CUDA ainda precisa, na máquina do usuário, do runtime do CUDA 12 (cudart, cuBLAS, cuFFT) e do **cuDNN 9**, que sozinho tem cerca de 1,1 GB de DLLs (pacote `nvidia-cudnn-cu12` 9.27, 743 MB compactado).

## Custo de integrar (se a medição justificar)

- O app é um exe só, com sherpa-onnx e onnxruntime estáticos e presos à CPU (ADR-003). A GPU exige um **processo auxiliar** separado, ligado às DLLs do CUDA, que o app chama para transcrever e, se ele falhar, volta para a CPU.
- Componente opcional "Aceleração NVIDIA", baixado como um modelo: cerca de 0,5 GB do sherpa-onnx CUDA mais 1,5 GB de CUDA e cuDNN. Só para NVIDIA.
- O modelo pt-BR é **int8** (quantização dinâmica). No CUDA, parte dessas operações costuma voltar para a CPU, o que reduz o ganho. Não existe export fp32 do pt-BR para o sherpa-onnx; seria preciso exportar do `.nemo` com o NeMo.
- Referência: na CPU, o app já faz 16× a 19× o tempo real (1 h de reunião em cerca de 4 min).

## Medição

O mesmo exemplo `s9_parakeet`, compilado à parte contra as bibliotecas CUDA do sherpa-onnx (`SHERPA_ONNX_LIB_DIR` + feature `shared`). O pacote não traz o `onnxruntime.lib`, que foi gerado a partir da DLL com `dumpbin` e `lib`. O cuDNN 9.27 veio do PyPI. O modelo pt-BR int8 foi carregado uma vez e transcreveu os 75 pedaços do podcast de 65 min, como o app faz. O mesmo executável rodou com `--provider cuda` e `--provider cpu`.

| | CPU (4 threads) | GPU (RTX 4060 Ti, CUDA 12.8 + cuDNN 9) |
|---|---:|---:|
| Tempo (3.929 s de áudio) | **242 s (16,2×)** | 257 s (15,3×) |
| Carga do modelo | 1,9 s | 2,9 s |
| Uso da placa | — | média de 22%, pico de 55% |
| Memória da placa | — | 3,2 GB |
| Texto | 13.790 palavras | 13.756 palavras, 97,5% igual ao da CPU |

O `sherpa-onnx-offline.exe` pronto não serve para medir: ele decodifica todos os arquivos num lote só e pediu 14,7 GB, tanto na placa quanto na CPU.

**Por que não acelera:** o modelo é int8 com quantização dinâmica. No CUDA, essas operações rendem mal e voltam para a CPU, então a placa fica ociosa quase o tempo todo. Ainda aparecem pequenas diferenças no texto. Para ter ganho, seria preciso um modelo fp32 do pt-BR, que não existe: teria que ser exportado do `.nemo` com o NeMo, ocuparia cerca de 2,5 GB, exigiria mais uns 2 GB de CUDA e cuDNN e serviria só para NVIDIA.

## Decisão

- Sem opção de transcrever pela placa de vídeo. O critério (≥ 3× mais rápido com o mesmo texto) ficou longe: a placa deu 0,94×.
- Reabrir só se surgir um build do sherpa-onnx com DirectML (qualquer placa) ou um export fp32/fp16 do pt-BR que mostre ganho grande. Se isso acontecer, aproveitar os padrões de um app de referência: pacote da GPU baixado sob demanda, volta automática para a CPU se a placa falhar e um arquivo sentinela que desliga a opção depois de uma queda.
- `s9_parakeet --provider` fica no exemplo, para repetir a medição.
