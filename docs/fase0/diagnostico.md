# Fase 0 · Diagnóstico da máquina

> 30/10/2026 · máquina de desenvolvimento (principal)

## Máquina

| Item | Valor | Situação |
|---|---|---|
| SO | Windows 11 Pro 26H2, build 26300.9711, x64 | ✅ ≥ 19041 (stealth ok) · ✅ ≥ 20348 (loopback por processo disponível, S3e) |
| CPU | AMD Ryzen 5 5600X, 6 núcleos / 12 threads | ✅ AVX2 (Zen 3) |
| RAM | 31,9 GB (19,4 GB livres) | ✅ |
| Disco C: | 30,2 GB livres | ⚠️ Apertado para VS Build Tools + Rust + target/ |
| Disco F: (projeto) | 59,8 GB livres | ✅ |
| Virtualização no firmware | ligada | ✅ pré-requisito do Windows Sandbox |

## Ferramentas

| Ferramenta | Situação |
|---|---|
| Rust / cargo / rustup | ❌ não instalado |
| MSVC (cl, link, dumpbin) + Windows SDK | ❌ não instalado (sem Visual Studio / Build Tools) |
| Node | ✅ v24.22.1 · npm 12.2.0 |
| Git | ✅ 2.56.0 |
| FFmpeg | ✅ 8.1 full_build (Gyan, via winget) no PATH |
| WebView2 Runtime | ✅ 156.0.4314.50 |
| VC++ Redist x64 | ✅ 14.51 instalado (por isso **esta máquina não serve** de “máquina limpa” para o S2) |
| Windows Sandbox | ❌ não habilitado (`WindowsSandbox.exe` ausente; habilitar exige admin + reinício) |

## Repositório

- `git init` (branch `main`) e `.gitignore` (target, node_modules, models, recordings, *.onnx, *.wav, *.part).

## Verificação após instalação (30/10/2026)

| Item | Resultado |
|---|---|
| Rust | ✅ rustup 1.29.1 · rustc/cargo 1.99.1 · `stable-x86_64-pc-windows-msvc` (padrão) |
| VS Build Tools 2022 | ⚠️ 17.14 instalado, mas **só com MSBuild**: falta o workload `Microsoft.VisualStudio.Workload.VCTools` (MSVC + Windows SDK). `cargo run` de um hello world falha com `link.exe not found` |
| Windows Sandbox | ❌ `WindowsSandbox.exe` ausente. Há reinício pendente (`PendingFileRenameOperations`); último boot 26/10 |

**Atualização (30/10, 19:45):** workload VCTools adicionado → ✅ MSVC 14.44.35207 · Windows SDK 10.0.26100.0 · `dumpbin` disponível · `cargo run` de hello world compila e roda. Sandbox ainda ausente (aguardando reinício).

**Atualização (30/10, 20:00):** Windows Sandbox habilitado via `dism.exe` (o cmdlet `Enable-WindowsOptionalFeature` falha no PowerShell 7 com "Classe não registrada"). Teste automático por `.wsb` (rede desligada, pasta mapeada) abriu em ~45 s e confirmou um ambiente limpo, adequado ao S2:

| No Sandbox | Valor |
|---|---|
| Build | 10.0.26100 |
| VC++ Redist x64 | ❌ ausente |
| `vcruntime140.dll` / `msvcp140.dll` em System32 | ❌ ausentes |
| WebView2 Runtime | ✅ 156.0.4314.50 (vem com o Windows, como nas máquinas dos colegas) |

## Pendências para continuar a Fase 0

1. Instalar **Visual Studio Build Tools 2022** (workload “Desenvolvimento para desktop com C++”: MSVC v143 + Windows SDK). Exige admin.
2. Instalar **Rust** via `rustup` (toolchain `stable-x86_64-pc-windows-msvc`), por usuário.
3. Habilitar **Windows Sandbox** (admin + reinício) — necessário para S2 e para a regra 10.
4. Baixar o **Parakeet TDT 0.6b v3 int8** (~640 MB) e o `silero_vad.onnx` para o S1.
