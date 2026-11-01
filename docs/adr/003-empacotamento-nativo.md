# ADR-003 · Empacotamento nativo: sherpa-onnx + onnxruntime estáticos, CRT estático

- **Status:** aceito (Fase 0, spike S2) · 30/10/2026
- **Contexto:** plano revisado, L2 e R2. O sherpa-onnx roda dentro do exe; se faltar uma DLL, o Windows encerra o processo antes de qualquer código nosso (sem chance de mensagem amigável).

## Fatos medidos

- O crate `sherpa-onnx` 2.0.1 baixa libs pré-compiladas do GitHub do projeto. No Windows x64:
  - `static` (padrão): `sherpa-onnx-v2.0.1-win-x64-static-MT-Release-lib.tar.bz2` (~117 MB, só para build);
  - `shared`: `sherpa-onnx-v2.0.1-win-x64-shared-MT-Release-lib.tar.bz2` (~7,5 MB), copia as DLLs para `target/<perfil>`.
  - As duas são compiladas com **CRT estático** (`/MT`).
- Com o Rust no padrão (`/MD`), o link avisa `LNK4098: defaultlib 'LIBCMT' conflita…` e o exe depende de `VCRUNTIME140.dll`, `VCRUNTIME140_1.dll` e `api-ms-win-crt-*`.

`dumpbin /dependents` (exemplo `s1_asr`, release):

| Variante | DLLs não-sistema | Tamanho |
|---|---|---|
| Padrão (`/MD` + sherpa estático) | `VCRUNTIME140.dll`, `VCRUNTIME140_1.dll` | 18,1 MB |
| **A** estático + `+crt-static` | **nenhuma** (só kernel32, ntdll, advapi32, setupapi, dxgi, dbghelp, api-ms-win-core-*) | **18,4 MB** |
| **B** `shared` + `+crt-static` | `sherpa-onnx-c-api.dll` → `onnxruntime.dll` (as DLLs também só dependem do sistema) | 0,3 MB + 21,5 MB de DLLs |

Teste no **Windows Sandbox** (build 26100, sem VC++ Redist, rede desligada), transcrevendo a gravação pt-BR de 124 s:

| Variante | Resultado |
|---|---|
| Padrão `/MD` | ❌ `0xC0000135` (STATUS_DLL_NOT_FOUND) em 1,4 s, sem mensagem |
| **A** | ✅ carga 3,70 s · 9,8× tempo real · texto idêntico ao do host |
| **B** | ✅ carga 3,72 s · 10,0× tempo real · texto idêntico |

## Decisão

**Caminho A:** sherpa-onnx e onnxruntime linkados estaticamente e **CRT estático** (`.cargo/config.toml` → `target-feature=+crt-static` para `x86_64-pc-windows-msvc`). O exe não depende de nenhuma DLL fora do Windows e dispensa o VC++ Redist.

- O caminho **C** (delay-load + `LoadLibraryExW`) só faz sentido com DLLs próprias (B). Com A não há DLL nossa para faltar, então C **não foi implementado**.
- A feature `shared` continua disponível em `ouveai-core` como plano B (por exemplo, se um EDR implicar com um exe grande), e aí o C volta à mesa.

## Consequências

- O instalador leva um exe só (~18 MB de núcleo ASR + o app), dentro da meta de < 25 MB a confirmar na Fase 1.
- ~~A verificar na Fase 1~~ **Verificado no S5:** Tauri 2.12 + sherpa-onnx estático no mesmo exe (26 MB) linkam sem `LNK4098`. O `tauri-build` usa CRT híbrido: runtime C++ estático e UCRT dinâmico (`api-ms-win-crt-*`, que vem com o Windows 10+). O exe carrega o modelo e abre a janela no Sandbox sem o Redist. A regra 10 continua valendo quando entrarem os plugins na Fase 1.
- O AVX2 não é exigido no link: o onnxruntime escolhe as instruções da CPU na hora de rodar. O app só avisa (`is_x86_feature_detected!("avx2")`).
- Quem compila precisa baixar a lib estática (~117 MB) uma vez. Em rede restrita, use `SHERPA_ONNX_ARCHIVE_DIR` ou `SHERPA_ONNX_LIB_DIR` com uma cópia local.
