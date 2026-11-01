# Plano — OuveAI: transcritor local de reuniões e legendas (Windows)

## Contexto

O usuário quer um app desktop **leve e rápido** para transcrever reuniões (Teams/Meet/Zoom) em **pt-BR**, 100% local e gratuito. Fontes:
- `docs/inicio/prompt.txt`: especificação de um transcritor local com **sherpa-onnx + Parakeet TDT 0.6b v3 int8**. Gera texto, `.srt` e o **tempo de cada palavra medido de verdade** (nunca estimado). Também cobre áudio longo, comparação com Whisper (Parte 4) e legenda de vídeo editado via `edl.json` (Parte 5).
- `docs/inicio/linguagem.txt`: escolher entre Electron e Tauri pensando em pouca memória e inicialização rápida.
- Estudo de apps de transcrição semelhantes: aproveitamos ideias de produto, nenhum código.

Decisões confirmadas com o usuário:
- **Só Windows** (arquitetura preparada para macOS depois).
- **Sem ata/resumo com IA** por enquanto.
- **Todas as partes do prompt.txt (2 a 5)** entram no plano.
- **UI:** janela principal (histórico, busca, configurações) + **mini janela flutuante** (transcrição ao vivo e controles), invisível no compartilhamento de tela.

Resultado esperado: instalador pequeno, app que abre em menos de 1 s, grava as duas vozes da reunião (EU e OUTROS) em tempo quase real, guarda tudo em SQLite com busca, e exporta `.json/.txt/.srt` com tempo por palavra.

---

## Decisão de stack: **Tauri v2** (não Electron)

| Critério | Tauri v2 | Electron |
|---|---|---|
| RAM em repouso | ~30–60 MB (WebView2 do sistema) | ~150–300 MB (Chromium embutido) |
| Inicialização | < 1 s | 1–3 s |
| Instalador | ~10–25 MB (sem o modelo) | ~80–150 MB |
| Invisível no compartilhamento | `content_protected(true)` → `WDA_EXCLUDEFROMCAPTURE` | `setContentProtection(true)`, mesmo mecanismo |
| Áudio nativo / ASR | Rust direto: `cpal`/`wasapi` + crate oficial `sherpa-onnx` | precisa de addon nativo ou helper C++ |

Apps de transcrição em Electron costumam pagar esse custo: helper C++ separado, servidor local e bundle grande. No Tauri o "helper nativo" já é o próprio backend em Rust.

**Stack final**
- Shell: Tauri v2 (Rust stable, MSVC). Plugins: `single-instance`, `global-shortcut`, `dialog`, `fs`, `shell` (sidecar FFmpeg), `log`, tray nativo.
- Frontend: **Svelte 5 + Vite + TypeScript** (bundle pequeno, sem runtime pesado). CSS próprio com tokens e sem framework de UI.
- Áudio: `cpal` (microfone) + loopback WASAPI (pelo `cpal`, abrindo stream de entrada no dispositivo de saída, ou pelo crate `wasapi`; decidir no spike) + `rubato` (resample para 16 kHz mono).
- ASR/VAD: crate oficial **`sherpa-onnx`** (bindings Rust sobre a C API; tem exemplo oficial `nemo_parakeet.rs`). Modelo Parakeet TDT 0.6b v3 int8 + `silero_vad.onnx`.
- Banco: `rusqlite` com feature `bundled` (FTS5 incluso), modo WAL.
- Diff de palavras (Partes 4 e 5): crate `similar` (alinhamento por sequência, tipo Myers/LCS).
- Arquivos de mídia: **FFmpeg como sidecar baixado sob demanda** (não vai no instalador). É usado só para importar arquivo e para o `silencedetect`.

---

## Ideias de produto que aproveitamos (e como)

| Ideia | Como fica no OuveAI |
|---|---|
| **Modo Stealth**: `setContentProtection`, `skipTaskbar`, título vazio | `content_protected(true)` nas duas janelas (configurável, ligado por padrão na mini), `skip_taskbar` na mini, título vazio opcional. Atalho global para mostrar/ocultar (ex.: `Ctrl+Shift+O`) e ícone na bandeja, para nunca "perder" o app. Avisar que no Windows anterior ao 10 2004 a janela sai preta em vez de invisível. |
| **Áudio dual EU/OUTROS** | Dois streams independentes, microfone = `EU` e loopback do sistema = `OUTROS`, cada um com seu próprio pipeline de VAD e ASR. Sem driver virtual e sem trocar o dispositivo de som do usuário. |
| **Fallback de dispositivo** | Se o fone/dispositivo cair, reabrir no dispositivo padrão sem encerrar a sessão, e mostrar aviso na UI. |
| **Não travar com a permissão de microfone** | Ler a permissão em `HKCU\...\CapabilityAccessManager\ConsentStore\microphone` pelo crate `winreg`, fora da thread principal, com cache de 1 s. Se estiver negada, mostrar como liberar. |
| **Leases/refcount do stream** | Mais simples: um `AudioEngine` dono dos streams, e eles só existem enquanto houver sessão gravando. |
| **VAD em duas camadas RMS + Silero** | Porta de energia RMS barata antes do Silero VAD do sherpa-onnx. Fecha o turno após ~800 ms de silêncio e corta à força segmentos maiores que 20 s (inspirado no `CappedAudioBuffer` de 15 s). |
| **Filtro de alucinação** | Descartar segmentos com repetição cíclica do mesmo token/trecho ou texto vazio vindo de ruído. |
| **Idle TTL do modelo** | Carregar o Parakeet só quando for gravar ou transcrever, e liberar depois de X minutos sem uso (padrão 5 min, configurável). Mantém a RAM em repouso baixa. |
| **Download com `.tmp` + verificação + checagem de RAM** | Model manager: confirmação antes de baixar (nome, origem, tamanho), barra de progresso, `.tmp` → rename atômico, conferência de tamanho/SHA-256, checagem de disco e RAM livres. |
| **Erros NTSTATUS** | Mapear `0xC0000135` (falta VC++ Redist / DLL) e `0xC000001D` (CPU sem AVX2) para mensagens claras. |
| **SQLite + FTS5 + WAL + cascade** | Mesmo desenho, adaptado: `sessions`, `segments`, `words`, `segments_fts` sincronizada por triggers. |
| **Sessões órfãs no boot** | Ao iniciar, sessões `recording` viram `interrupted`, com o texto preservado. O que já foi transcrito é gravado a cada segmento, nunca só no final. |
| **Instância única** | `tauri-plugin-single-instance`: a segunda instância foca a primeira. |
| **Não aplicar update durante gravação** | Se um auto-update entrar no futuro, ele não reinicia o app no meio de uma gravação. Updater fica fora do MVP. |
| **Atalhos claros em modo stealth** | Bandeja + atalhos globais: gravar/parar, mostrar/ocultar a mini, abrir a principal. |

**Não aproveitar:** servidor Express TLS, Firebase/auth/paywall, MCP, LLM local, captura de tela, overlay de tela inteira com `setShape`, telemetria Sentry/PostHog. Nada disso é necessário e tudo pesa.

---

## Arquitetura

```
┌──────────── Frontend (Svelte, WebView2) ────────────┐
│ main window: Sessões · Busca · Detalhe/Editor ·     │
│              Importar arquivo · Legendas/EDL ·      │
│              Configurações · Modelos                 │
│ mini window: ao vivo (EU/OUTROS), ●REC, pausar, parar│
└──────────── invoke() / events ───────────────────────┘
┌──────────── src-tauri (Rust) ───────────────────────┐
│ commands/   → camada fina que chama o core          │
│ audio/      → mic (cpal) + loopback WASAPI, resample│
│ live/       → pipeline por canal: RMS→Silero→ASR    │
│ stealth/    → content protection, skip taskbar      │
│ tray, shortcuts, single-instance                    │
└──────────── usa ─────────────────────────────────────┘
┌──────────── crates/ouveai-core (Rust lib) ──────────┐
│ asr/       → wrapper sherpa-onnx (Parakeet, Whisper)│
│ words/     → REGRAS DE TEMPO (tokens→palavras)      │
│ subs/      → segmentação única + SRT/TXT/JSON       │
│ media/     → FFmpeg: decode p/ WAV 16k, silencedetect,│
│              corte de áudio longo                    │
│ compare/   → diff Parakeet × Whisper (Parte 4)      │
│ edl/       → edl.json → SRT + verificação (Parte 5) │
│ store/     → SQLite (rusqlite) + migrações          │
│ models/    → download/verificação/caminhos           │
└──────────────────────────────────────────────────────┘
┌──────────── crates/ouveai-cli (bin) ────────────────┐
│ equivalente aos comandos do prompt.txt:              │
│ ouveai transcribe arquivo.mp4 [--check]              │
│ ouveai transcribe --from-json arquivo.json           │
│ ouveai compare arquivo.mp4      (Parte 4)            │
│ ouveai edl-subs edl.json [--verify final.mp4] (P5)   │
└──────────────────────────────────────────────────────┘
```

O `ouveai-core` não depende do Tauri. Assim a CLI cumpre o prompt.txt ao pé da letra e a lógica crítica (tempos, segmentação, EDL) fica testável sem UI.

**Estrutura de pastas (raiz `F:\Projetos\OuveAI`)**
```
Cargo.toml (workspace)        package.json / vite.config.ts
crates/ouveai-core/           crates/ouveai-cli/
src-tauri/ (app Tauri)        src/ (Svelte: routes main + mini)
docs/ (existente) + docs/adr/ (decisões novas)
```
Dados do usuário em `%APPDATA%\OuveAI\`: `ouveai.db`, `models\`, `bin\ffmpeg.exe`, `recordings\<sessão>\eu.wav|outros.wav` (opcional), `logs\`.

---

## Regras centrais (valem para todo o código)

1. **Tempo por palavra sempre medido** (prompt.txt): vem dos timestamps/durations do modelo ou da análise do áudio (Silero VAD ao vivo, `silencedetect` do FFmpeg em arquivo). Nunca dividir frase, interpolar nem ajustar à mão.
2. **Montagem das palavras** (`core::words`): token com `▁` (ou espaço, conforme o que a versão instalada devolver) abre palavra nova, e token sem marcador gruda na anterior. Início = timestamp do 1º token. Fim = início + duração do último token, se houver `durations`. Se não houver, fim = início da próxima palavra recuado ao começo do silêncio medido entre elas, limitado a início + 1,5 s, e a última palavra fica limitada à duração do áudio.
3. **Pausas** = silêncio medido (`silencedetect -35 dB, 0,3 s` em arquivo, segmentos do VAD ao vivo), nunca a diferença entre inícios.
4. **Segmentação de legenda única** (`core::subs::segment`) com config no topo: `MAX_CHARS=36, PAUSA_QUEBRA=0.45, VIRGULA_MIN_CHARS=20, SAIDA_APOS_FIM=0.1, MIN_DURACAO=1.0, INTERVALO_MIN=0.08`, com as regras de quebra do prompt.txt. Usada no ao vivo, na importação de arquivo e no EDL.
5. **Não inventar API**: antes de codar contra o sherpa-onnx, ler o README/docs.rs da versão instalada e o exemplo oficial `nemo_parakeet.rs`, e usar os nomes de campo exatos.
6. **Áudio nunca sai da máquina.** Nenhuma chamada de rede além dos downloads confirmados de modelo e FFmpeg.

---

## Fases de execução

> Regra para o agente executor: **parar ao fim de cada fase** com um resumo ✅ e esperar o "ok" do usuário. **Pedir confirmação antes de cada instalação ou download** (dizer o que é, de onde vem e o tamanho). Nada global nem como administrador sem autorização. Se algo falhar, mostrar o erro e propor a correção, **sem trocar biblioteca, modelo ou abordagem sem perguntar**. Responder sempre em pt-BR.

### Fase 0 — Diagnóstico e spike técnico
- Informar SO, CPU (modelo/núcleos), RAM, disco livre (precisa de ~2 GB para o Parakeet e +3 GB para o Whisper da Parte 4). Verificar Rust/MSVC, Node LTS, WebView2 e FFmpeg no PATH.
- `git init` na raiz e criar `.gitignore` (models, target, node_modules, recordings).
- **Spike em `crates/ouveai-core`** (exemplo `cargo run --example spike`):
  1. Carregar o Parakeet v3 int8 via crate `sherpa-onnx` (baixar de `github.com/k2-fsa/sherpa-onnx/releases/download/asr-models/sherpa-onnx-nemo-parakeet-tdt-0.6b-v3-int8.tar.bz2`, ~640 MB, com confirmação) e transcrever um WAV pt-BR curto.
  2. **Relatar quais campos o resultado traz** (`tokens`, `timestamps`, `durations`?) e qual é o marcador de palavra. Se o crate Rust não expuser timestamps, parar e trazer opções ao usuário (ex.: C API via FFI direto).
  3. Validar a DLL nativa no Windows (onnxruntime/sherpa) ao lado do exe.
  4. Abrir o loopback WASAPI e o microfone ao mesmo tempo por 10 s, gravando dois WAVs, e escolher `cpal` ou `wasapi`.
  5. Medir o tempo de carga do modelo e o fator de tempo real.
- **Pronto quando:** um relatório com os campos reais da API e uma decisão de captura registrada em `docs/adr/001-stack.md`.

### Fase 1 — Esqueleto do app Tauri
- Workspace Cargo (`ouveai-core`, `ouveai-cli`, `src-tauri`) e Svelte 5 + Vite + TS.
- Duas janelas no `tauri.conf.json`: `main` (normal) e `mini` (≈360×220, sem moldura, `always_on_top`, `skip_taskbar`, arrastável, posição salva).
- `stealth`: comando `set_stealth(on)` aplicando `set_content_protected` nas duas janelas. Persistido nas configurações, ligado por padrão.
- Bandeja (abrir, gravar/parar, stealth on/off, sair), atalhos globais e instância única.
- Logs em arquivo (`tauri-plugin-log`) e tela de Configurações básica.
- **Pronto quando:** o app abre em < 1 s, a RAM em repouso fica < 80 MB (medir no Gerenciador de Tarefas: processo + WebView2) e as duas janelas **não aparecem** num print (Ferramenta de Captura), no OBS e no compartilhamento do Teams/Meet, mas continuam visíveis no monitor.

### Fase 2 — Modelos e FFmpeg sob demanda
- `core::models`: catálogo (Parakeet v3 int8, Silero VAD, e na Parte 4 o Whisper large-v3 do sherpa-onnx), download com confirmação, progresso por evento, `.tmp` → rename, conferência de tamanho/hash, extração `.tar.bz2` (crates `bzip2` + `tar`) e remoção do pacote depois.
- Checagem de disco e RAM antes de baixar. Primeiro uso: assistente "Baixar modelo".
- FFmpeg: usar o do PATH se existir. Se não, oferecer baixar uma build estática para `%APPDATA%\OuveAI\bin` (com confirmação e tamanho).
- **Pronto quando:** download interrompido não deixa modelo corrompido e a UI mostra o estado de cada modelo.

### Fase 3 — Núcleo de transcrição de arquivo (Partes 2 e 3 do prompt.txt)
- `core::media`: FFmpeg → WAV temporário 16 kHz mono PCM16 (apagado no fim), `silencedetect` (-35 dB, 0,3 s) e corte de áudio > 10 min em silêncio entre 9:00 e 10:30. Sem silêncio na janela, cortar no ponto de menor volume medido e **avisar**. Somar o offset de cada pedaço aos tempos.
- `core::asr`: reconhecedor offline `nemo_transducer`, CPU, `threads = min(4, núcleos)`.
- `core::words` (regras acima) e `core::subs` (segmentação + `HH:MM:SS,mmm`).
- Saídas UTF-8 ao lado do arquivo: `.json` `{"source","duration","words":[{word,start,end}],"pauses":[{start,end}]}` com 3 casas, `.txt` e `.srt`. Modos `--check` (gera só `.check.json`) e `--from-json` (regenera `.txt`/`.srt`).
- CLI `ouveai transcribe` e tela "Importar arquivo" na UI (arrastar e soltar, progresso, botões abrir/exportar).
- Editor de texto: corrige só `word`, nunca `start`/`end`, e regenera.
- Relatório no fim: tempo de carga, tempo de transcrição, fator de tempo real e lista de trechos suspeitos (nomes próprios, termos técnicos, grafia de português europeu) com tempo e erros/min. Acima de 3/min, sugerir a Parte 4.
- **Testes unitários** de `words` (tokens sintéticos com e sem `durations`, pontuação grudando, 1º token) e de `subs` (cada regra de quebra, MIN_DURACAO, INTERVALO_MIN).
- **Pronto quando:** um vídeo de 1–2 min gera os 3 arquivos, a legenda sincroniza no VLC (`.srt` com o mesmo nome) e o fator de tempo real é informado.

### Fase 4 — Captura ao vivo e transcrição de reunião
- `audio`: mic e loopback em threads próprias, conversão para mono, `rubato` para 16 kHz, ring buffer lock-free (`ringbuf`) entre o callback de áudio e o pipeline, sem bloquear o callback.
- Fallback de dispositivo e eventos `device-changed`. Permissão de microfone via registro (`winreg`).
- `live`: por canal, RMS → Silero VAD (sherpa-onnx) → segmento (fecha com ~800 ms de silêncio ou no máximo 20 s) → Parakeet → `core::words` com offset do início do segmento → filtro de alucinação → evento `segment` para as duas janelas + gravação no SQLite.
- Um único reconhecedor compartilhado, com fila de segmentos por canal (o Parakeet int8 em CPU é bem mais rápido que tempo real).
- Opção "salvar áudio da reunião" (WAV por canal) para reprocessar depois e para os `.srt`.
- **Eco** (sem fone, o microfone capta a voz dos OUTROS): no MVP, detectar segmento do mic sobreposto no tempo a um segmento de OUTROS com texto muito parecido (`similar` ≥ 0,8) e descartar. Recomendar fone na UI. AEC de verdade fica como melhoria futura.
- Pausar/retomar/parar. Idle TTL do modelo.
- **Pronto quando:** numa chamada real de 10 min (Teams ou Meet), as falas aparecem rotuladas EU/OUTROS na mini com atraso de poucos segundos, tudo fica salvo, e desconectar o fone no meio não derruba a sessão.

### Fase 5 — Persistência, histórico e busca
- Schema (migrações versionadas em `core::store`):
  - `sessions(id, title, kind['meeting'|'file'], source_path, started_at, ended_at, status['recording'|'ended'|'interrupted'], duration, audio_dir)`
  - `segments(id, session_id FK CASCADE, speaker['EU'|'OUTROS'|'ARQUIVO'], start, end, text)`
  - `words(id, segment_id FK CASCADE, idx, word, start, end)`
  - `segments_fts` (FTS5 `content=segments`, tokenizer `unicode61 remove_diacritics 2`) + triggers ai/ad/au.
- PRAGMAs `foreign_keys=ON` e `journal_mode=WAL`. Sessões órfãs no boot → `interrupted`.
- UI principal: lista de sessões, busca com destaque, detalhe com linha do tempo EU/OUTROS (clicar numa fala toca o áudio, se houver), renomear, excluir, exportar `.txt/.srt/.json/.md`.
- **Pronto quando:** matar o processo durante a gravação preserva o texto até o último segmento, e a busca acha palavras sem diferenciar acento.

### Fase 6 — Parte 4: comparação com Whisper large-v3 (opcional, sob pedido)
- **Antes de instalar, propor a forma ao usuário e esperar a escolha.** Proposta padrão: Whisper large-v3 em ONNX pelo próprio sherpa-onnx (sem Python), `language="pt"`, cortes ≤ 30 s em silêncio medido.
- `core::compare`: rodar os dois no mesmo áudio, alinhar as listas de palavras por sequência (`similar`, texto normalizado) e mostrar só os trechos divergentes, com o tempo. O resultado final usa o texto que o usuário escolher e **os tempos do Parakeet**.
- UI: tela lado a lado com escolha por trecho. CLI `ouveai compare`.

### Fase 7 — Parte 5: legenda de vídeo editado (EDL)
- `core::edl`: lê `edl.json` (`file, in, out, position`) e valida (`out > in`, sem sobreposição no vídeo final, que é erro). Lacunas entre cortes são permitidas e listadas uma vez. O `.json` de mesmo nome-base é obrigatório, com erro claro se faltar.
- Palavra entra se `start ∈ [in, out)`. Tempo final = tempo no bruto − in + position, e fim que passa de `out` é cortado em `out`. Mesma `segment()`, e nenhum bloco atravessa um corte.
- Verificação: `transcribe --check` no vídeo final, alinhamento por sequência com texto normalizado (minúsculas, sem pontuação nem acento), só pares casados. Informar desvio mediano e máximo dos inícios e palavras sem par (referência 0,12 s). Listar desvios > 0,3 s com causa provável (`in`/`position` errados) e propor a correção **na origem** (`edl.json`), sem empurrar tempos.
- UI "Legendas" (carregar EDL, validar, gerar, verificar) + CLI `ouveai edl-subs`.

### Fase 8 — Empacotamento e polimento
- `tauri build` → instalador NSIS por usuário (sem admin), com DLLs do sherpa-onnx/onnxruntime incluídas e o modelo fora do instalador.
- Medições finais: tamanho do instalador, tempo de abertura a frio, RAM em repouso, RAM gravando e CPU média gravando.
- Revisão de UX da mini (contraste, legível por cima de vídeo) e README de uso em pt-BR.

---

## Verificação (fim a fim)

1. `cargo test -p ouveai-core`: testes de `words`, `subs`, `edl` (validação, mapeamento de tempo, blocos não atravessam corte) e `compare` (alinhamento).
2. CLI: `ouveai transcribe teste.mp4` → `.json/.txt/.srt`, abrir no VLC e conferir a sincronia; `--from-json` após editar uma palavra; `--check` não toca no `.srt`.
3. App: `npm run tauri dev`, depois uma reunião real de teste no Teams/Meet com fone, depois sem fone (eco), e trocar de dispositivo no meio.
4. Stealth: compartilhar a tela inteira no Meet/Teams e gravar no OBS. As janelas não aparecem na transmissão, mas aparecem no monitor.
5. Robustez: matar o processo gravando → reabrir → sessão `interrupted` com o texto.
6. Desempenho: abertura < 1 s, repouso < 80 MB, fator de tempo real informado.
7. Parte 5: EDL de exemplo com 3 cortes → `.srt` final → verificação com desvio mediano ≤ ~0,12 s.

## Riscos e pontos de atenção
- **Timestamps no crate Rust do sherpa-onnx**: confirmar na Fase 0. Plano B é a C API via FFI, decidido com o usuário.
- **Content protection**: depende do Windows 10 2004+. Algumas ferramentas corporativas de gravação em nível de driver conseguem ignorar (risco conhecido). Avisar na UI.
- **Eco sem fone**: a mitigação por similaridade é imperfeita. Recomendar fone.
- **Parakeet v3 em pt-BR**: pode trazer grafia de português europeu e errar nomes próprios. Mitigação: editor de palavras + Parte 4. Vocabulário/hotwords fica como melhoria futura, se a API suportar.
- **Ética**: o modo invisível é para privacidade do usuário. Considerar um aviso na UI lembrando de informar os participantes de que a reunião está sendo transcrita (LGPD).
