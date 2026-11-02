# Plano Revisado — OuveAI

> **Versão 2 · 30/10/2026** · Revisão de arquitetura e de gestão de projeto sobre o [PLANO.md](PLANO.md) (v1) e o [oportunidade.md](oportunidade.md).
> Este documento **substitui o PLANO.md como referência de execução**. O PLANO.md fica como histórico e, onde os dois divergirem, vale este.
> Itens marcados com **(D1)…(D7)** dependem de uma decisão sua (seção 12). Até você responder, vale a recomendação indicada.

---

## 0. Sumário executivo

O plano original acerta na stack (Tauri v2 + Rust + sherpa-onnx), nas regras de tempo por palavra e na escolha das ideias de produto aproveitadas. Esta revisão mantém essa base. O que muda é principalmente **ordem, riscos e distribuição**:

1. **Primeiro sai o app de reunião (v1.0).** As Partes 4 (Whisper) e 5 (EDL) continuam no plano, mas vêm depois do piloto. O escopo é o mesmo, só a ordem muda. **(D1)**
2. **O empacotamento sai do fim e vai para o começo.** Já na Fase 1 existe um instalador mínimo, testado numa máquina limpa (Windows Sandbox). O maior risco de "não abre no PC de outra pessoa" é DLL/runtime C++, e no OuveAI esse erro derruba o processo antes de qualquer mensagem nossa aparecer.
3. **Os dados vão para `%LOCALAPPDATA%`, não para `%APPDATA%`.** O `%APPDATA%` é o perfil Roaming: em empresa, 640 MB de modelo mais os WAVs deixam o logon lento e estouram a cota.
4. **Captura de OUTROS confiável no Teams, no Meet e no Zoom.** O app capta os dois dispositivos padrão do Windows (multimídia e comunicação), acompanha a troca de dispositivo (inclusive a troca de perfil do Bluetooth) e usa relógio QPC, porque o loopback para de mandar pacotes quando nada está tocando. A mini ganha um medidor de sinal por canal e um aviso quando OUTROS fica mudo. A captura por aplicativo (Windows 11) entra como opção avançada. **(D7)**
5. **Download que funciona na rede da empresa.** Usa os certificados do Windows e o proxy do sistema, retoma downloads interrompidos e, principalmente, oferece **"baixar pelo navegador / importar arquivo"**, que passa por qualquer proxy.
6. **Eco revisado.** Sem fone, a fala suspeita de eco é **marcada, não apagada**. Com fone, não há filtro nenhum. O tipo de saída (fone ou alto-falante) é detectado sozinho, com ajuste manual.
7. **Atalho `Ctrl+Shift+O` trocado.** No Teams ele liga e desliga a câmera. Os novos atalhos padrão não conflitam com Teams, Zoom nem Meet, e podem ser trocados.
8. **Utilidades de reunião:** copiar a transcrição em Markdown, marcador ⭐ durante a reunião e política de retenção do áudio. **(D2)**
9. **Suporte sem telemetria:** "Exportar diagnóstico" (logs e ambiente, sem texto nem áudio), e os logs nunca guardam o que foi transcrito.
10. **Inconsistência corrigida:** a Fase 4 gravava num SQLite cujo schema só era criado na Fase 5. O banco mínimo passa para a Fase 3.

---

## 1. Avaliação dos documentos de origem

### 1.1 PLANO.md: o que fica como está

- Tauri v2 + Svelte 5 + Rust, com o `ouveai-core` independente do Tauri e uma CLI que segue o prompt.txt ao pé da letra.
- Regras de tempo por palavra, pausas medidas e segmentação única de legenda, fiéis ao prompt.txt.
- A seleção de ideias de produto: stealth, áudio dual EU/OUTROS, VAD em duas camadas, idle TTL do modelo, sessões órfãs, FTS5.
- A disciplina de execução: parar a cada fase, pedir confirmação antes de baixar, nunca trocar de abordagem sem perguntar.

### 1.2 PLANO.md: lacunas encontradas nesta revisão

| # | Lacuna | Por que importa | Ajuste |
|---|---|---|---|
| L1 | Dados em `%APPDATA%` (Roaming) | Em perfil corporativo o Roaming é sincronizado com o servidor: modelo e WAVs deixam o logon lento e estouram a cota | Tudo em `%LOCALAPPDATA%` (seção 6.6) |
| L2 | "Mapear `0xC0000135` / `0xC000001D` para mensagens claras" | Esse mapeamento pressupõe o ASR como **processo filho**, com o app lendo o código de saída. No OuveAI o sherpa-onnx roda **dentro do exe**: se faltar uma DLL, o Windows nem abre o app e nossa mensagem nunca aparece | A forma de linkar as bibliotecas nativas é decidida no spike (ADR-003), e o app é testado em máquina limpa desde a Fase 1. Sobre AVX2: o onnxruntime escolhe as instruções da CPU na hora de rodar, então basta um aviso com `is_x86_feature_detected!` |
| L3 | A Fase 4 grava no SQLite, mas o schema só nasce na Fase 5 | Dependência na ordem errada | Banco mínimo na Fase 3 |
| L4 | O loopback WASAPI não manda pacotes enquanto nada toca | Medir o tempo contando amostras encolhe a linha do tempo de OUTROS e desalinha EU e OUTROS | Relógio da sessão pelo QPC de cada pacote, com silêncio preenchendo as lacunas. Validar no spike |
| L5 | Atalho global `Ctrl+Shift+O` | No Teams é liga/desliga câmera, e o atalho global "rouba" a tecla do Teams | Novos padrões (Fase 1) |
| L6 | Meta de RAM < 80 MB com duas WebViews | Cada janela tem seu próprio processo de renderização do WebView2 | Janelas criadas sob demanda e metas calibradas na Fase 0 (seção 4) |
| L7 | Entregar o app a outras pessoas | Um exe sem assinatura cai no SmartScreen, e o antivírus/EDR da empresa pode bloquear um app que capta áudio, registra atalhos globais e se esconde da captura de tela | Falar com o TI antes do piloto, publicar o SHA-256 do instalador e assinar se o TI exigir **(D4)** |
| L8 | Suporte | Sem telemetria (o que está certo), não há como investigar um problema no PC de quem usa | "Exportar diagnóstico" e logs sem conteúdo |
| L9 | `threads = min(4, núcleos)` também ao vivo | Disputa CPU com o Teams com vídeo e faz o áudio engasgar | Ao vivo, 2 threads (configurável) e threads de captura com MMCSS. Arquivo continua com `min(4, núcleos)` |
| L10 | A gravação começa com o modelo descarregado | A carga leva segundos, e o começo da fala se perderia | Captura e VAD começam na hora, e os segmentos esperam na fila até o modelo carregar |
| L11 | O ao vivo só pode ser testado numa reunião real | Lento e impossível de repetir igual | Fonte de áudio "replay" (dois WAVs) no mesmo pipeline e comando `ouveai replay` |
| L12 | Licenças | O Parakeet v3 é CC-BY-4.0, que exige atribuição | Tela "Sobre / licenças" |
| L13 | FFmpeg como "sidecar" pelo plugin `shell` | No Tauri, sidecar é binário que vai no instalador. Aqui o FFmpeg é baixado depois e precisa rodar também na CLI | O `core::media` chama o FFmpeg direto (`std::process::Command` com `CREATE_NO_WINDOW`, para não piscar console). Sai o plugin `shell` |
| L14 | "Conferência de SHA-256" | Os releases do sherpa-onnx não publicam hash | Calcular o hash na Fase 0 e fixar no catálogo (URL, tamanho e SHA-256) |
| L15 | "Fator de tempo real" sem definição | O prompt.txt define como duração ÷ tempo de transcrição (maior = mais rápido), o inverso do RTF de costume | Usar a definição do prompt em todo lugar, rotulada como "N× mais rápido que o tempo real" |
| L16 | "Trechos suspeitos" | No prompt isso era julgamento do agente. No app precisa de um critério testável | Heurísticas explícitas (Fase 3), apresentadas como "candidatos" |
| L17 | Corte forçado em 20 s | Cortar no meio da palavra piora o reconhecimento | Cortar no ponto de menor energia dos últimos ~2 s |
| L18 | Importar arquivo durante uma reunião | Só existe um reconhecedor | A reunião tem prioridade e a importação espera na fila, com aviso |
| L19 | Posição salva da mini | Se o monitor foi desconectado, a janela abre fora da tela | Validar a posição contra os monitores atuais. O atalho de resgate centraliza a janela |
| L20 | Clicar na mini | Rouba o foco do Teams, e os atalhos dele (mudo etc.) param de responder | Avaliar janela que não pega foco (`WS_EX_NOACTIVATE`) na Fase 1 |
| L21 | OUTROS é uma voz só | Separar quem falou (diarização) é o próximo pedido natural | Coluna `speaker_label` já no schema (custo zero). A diarização fica no backlog |
| L22 | Apagar dados | Exigência de LGPD e confiança de quem usa | "Excluir sessão", "Excluir tudo" e um desinstalador que pergunta se apaga os dados |
| L23 | Colunas `start`/`end` no SQLite | `END` é palavra reservada do SQL | Renomear para `start_s`/`end_s` |

### 1.3 oportunidade.md: veredito por item

| Oportunidade | Veredito | Onde entra |
|---|---|---|
| O1 Distribuição "zero-setup" | **Aceita com ajustes** | Fase 0 (S2, S4), Fase 1, Fase 2, Fase 6 |
| O2 Captura WASAPI Teams/Zoom/Meet | **Aceita com correção técnica** | Fase 0 (S3), Fase 4 |
| O3 Refinamento do stealth | **Aceita em parte** (os atalhos sugeridos conflitam) | Fase 1 |
| O4 Foco em reunião antes do EDL | **Aceita** (muda a ordem, não o escopo) | Seção 3, Fases 5 a 8 |
| O5 Eco realista | **Aceita com ajuste** (marcar em vez de apagar) | Fase 4 |

**O1: Zero-setup**
- *Aceito:* assistente visual com %, velocidade e tempo restante; download que retoma (`Range`) com novas tentativas; importar o modelo de um arquivo ou pasta de rede; instalador NSIS por usuário (que já estava no PLANO).
- *Ajustado:*
  - **Runtime VC++:** o diagnóstico está certo, mas a solução está incompleta (ver L2). Colocar as DLLs do VC++ ao lado do exe (o que a Microsoft permite) é **uma** das opções do ADR-003, não a decisão.
  - **TLS nativo resolve o certificado, não o proxy.** A inspeção TLS do Zscaler/Netskope instala o certificado raiz no repositório do Windows, e validar por esse repositório resolve essa parte. O proxy é outra camada: o app lê o proxy do sistema e tem um campo manual, mas PAC/WPAD e proxy com login integrado (NTLM/Kerberos) não ficam cobertos. Por isso o caminho **"baixar pelo navegador e importar"** é obrigatório, não opcional.
  - **Esforço:** médio, não baixo.
- *Novo:* hash fixado no catálogo (L14) e um espelho alternativo, se existir (confirmar na Fase 0). Com o hash conferido, a origem do arquivo deixa de importar.

**O2: Captura multi-app**
- *Aceito:* seletor de dispositivo de saída (Automático ou dispositivo específico) e recuperação de `AUDCLNT_E_DEVICE_INVALIDATED`.
- *Corrigido:* não existe API que diga "qual papel está ativo agora" (multimídia ou comunicação). O modo **Automático** passa a captar o loopback do padrão multimídia **e** do padrão de comunicação, somando os dois (um só, quando forem o mesmo dispositivo), e a acompanhar as trocas de padrão (`IMMNotificationClient`). No Bluetooth, a troca para o perfil de chamada (HFP) não é só "cai e volta": o som passa para **outro endpoint** ("Headset / Viva-voz"), que costuma ser o padrão de comunicação. Captar os dois papéis cobre esse caso.
- *Novo:*
  - **Loopback por processo** (Windows 11 / build 20348+) como modo "Por aplicativo": capta só o Teams, o Zoom ou o navegador, seja qual for o dispositivo, e ignora notificações e música. Ressalva: no navegador ele pega todas as abas. Fica como opção avançada se o spike for simples. **(D7)**
  - **Medidor de sinal por canal**, com o aviso "não estou ouvindo os outros participantes" quando OUTROS fica mudo.
  - **Lacunas do loopback em silêncio** (L4).
  - **Não abrir o microfone antes de gravar:** abrir o mic de um fone Bluetooth força o perfil HFP e piora o som que o usuário ouve.

**O3: Stealth**
- *Aceito:* checagem do build do Windows (abaixo de 19041, a proteção começa desligada e o app avisa). Custa pouco, mas o impacto é pequeno, porque o Windows 10 anterior ao 2004 já está fora de suporte. Bandeja com menu e atalho de resgate já estavam no PLANO. O atalho agora também **centraliza** a mini no monitor atual.
- *Rejeitado:* os atalhos sugeridos. `Alt+Shift` é a troca de idioma do teclado no Windows, e `Ctrl+Shift+Space` é espaço não separável no Word. E o `Ctrl+Shift+O` do PLANO conflita com a câmera do Teams. Novos padrões na Fase 1.
- *Novo:* janela fora da tela (L19) e não roubar o foco (L20).

**O4: Rebalanceamento**
- *Aceito:* a v1.0 de reunião vem antes das Partes 4 e 5 **(D1)**. Entram copiar em Markdown, marcador ⭐ e retenção do áudio.
- *Ajustado:*
  - "Copiar ata" vira **"Copiar transcrição"**, para não confundir com a decisão "sem ata com IA" (aqui é só formatação).
  - **Marcador pela barra de espaço não funciona:** um atalho global no espaço quebraria a digitação em todo o Windows, e a mini quase nunca está com o foco. Fica um botão ⭐ na mini mais um atalho global configurável.
  - A conta de 230 MB/h está certa (16 kHz × 16 bits × 2 canais).
- *Novo:* o padrão passa a ser **não salvar o áudio**, com opção de retenção de 7 ou 30 dias e limpeza automática **(D2)**.

**O5: Eco**
- *Aceito:* a chave "Estou usando fone". Com fone, nenhum filtro de eco.
- *Ajustado:* o padrão da chave não é fixo em "ligado". Ele vem do **tipo do dispositivo de saída** (a propriedade de formato do endpoint diz se é fone/headset ou alto-falante), com ajuste manual, porque monitor HDMI e caixa USB podem enganar.
- *Rejeitado:* "sem fone, usar só o VAD enquanto o áudio remoto toca". Isso apagaria fala real do usuário sempre que duas pessoas falassem ao mesmo tempo.
- *Em vez disso:* comparar o envelope de energia do mic com o do loopback (correlação com atraso de até ~300 ms), mais sobreposição no tempo, mais similaridade de texto como desempate. O segmento suspeito é **marcado** como `eco_suspeito`: fica oculto por padrão e pode ser recuperado. Cancelamento de eco (AEC) de verdade continua no backlog.

**Matriz revisada**

| Item | Impacto | Esforço | Fase |
|---|:---:|:---:|:---:|
| Linkagem nativa + teste em máquina limpa (L2) | 🔴 Alto | Médio | 0 e 1 |
| Download corporativo + importar arquivo (O1) | 🔴 Alto | Médio | 0 e 2 |
| Captura de OUTROS multi-app + Bluetooth + relógio QPC (O2, L4) | 🔴 Alto | Alto | 0 e 4 |
| Dados em `%LOCALAPPDATA%` (L1) | 🔴 Alto | Muito baixo | 1 |
| Conversa com o TI / assinatura (L7) | 🔴 Alto | Baixo (técnico) | 6 |
| Atalhos sem conflito + resgate da janela (O3, L5, L19) | 🟡 Médio | Baixo | 1 |
| Copiar em Markdown + marcador ⭐ + retenção (O4) | 🟡 Médio | Baixo | 5 |
| Eco marcado + detecção de fone (O5) | 🟡 Médio | Médio | 4 |
| Exportar diagnóstico (L8) | 🟡 Médio | Baixo | 6 |

---

## 2. Contexto e decisões

**Mantidas (já confirmadas com você no PLANO.md)**
- Só Windows, com arquitetura preparada para macOS depois.
- Sem ata/resumo com IA.
- Partes 2 a 5 do prompt.txt dentro do plano.
- UI com janela principal (histórico, busca, configurações) e mini janela flutuante invisível no compartilhamento de tela.
- 100% local e gratuito. O áudio nunca sai da máquina.

**Públicos**
- **Usuário principal (você):** técnico, usa o app em reuniões e para legendar vídeo editado, e usa a CLI.
- **Usuários do piloto:** não técnicos, em notebook corporativo (Windows 10 22H2 ou 11, 8–16 GB de RAM, proxy, antivírus/EDR, sem admin). Querem transcrever reuniões de Teams, Meet e Zoom e copiar o texto.

**Requisitos mínimos propostos (D6):** Windows 10 2004 (build 19041) ou mais novo, x64, 8 GB de RAM, 4 núcleos, 2 GB livres em disco. ARM64 fica de fora da v1.0 (roda por emulação, mais devagar).

---

## 3. Escopo por release

| Release | Fases | Para quem | Conteúdo |
|---|---|---|---|
| **v0.1 "Arquivo"** | 0–3 | Você | CLI `transcribe` (Partes 2 e 3) + importar arquivo na UI |
| **v0.9 "Piloto"** | 4–6 | 3–5 pessoas | Reunião ao vivo EU/OUTROS, histórico, busca, exportar/copiar, marcadores, stealth, instalador |
| **v1.0 "Reunião"** | saída do piloto | Público | Correções do piloto |
| **v1.1 "Vídeo"** | 7–8 | Você | Parte 4 (Whisper, sob pedido) e Parte 5 (EDL) |

**Backlog (depois da v1.0, sem compromisso):** diarização dos OUTROS ("OUTROS 1, 2…", pelo sherpa-onnx, exige áudio salvo); dicionário de correções do usuário e hotwords (se o TDT suportar, verificar na Fase 0); AEC de verdade; texto parcial ao vivo enquanto a pessoa ainda fala; banco criptografado (DPAPI/SQLCipher); áudio em FLAC; ARM64 nativo; macOS; atualização automática assinada.

**Fora do escopo:** resumo com IA, nuvem, telemetria, contas e login.

---

## 4. Metas não funcionais

> Metas de RAM e abertura **calibradas pelo S5** (30/10/2026, app Tauri 2.12 vazio, Ryzen 5 5600X): bandeja 1,4 MB · 1 janela 70 MB (7 processos) · 2 janelas 88 MB · abertura até a página carregada 383 ms (mediana), 642 ms na 1ª execução · no Sandbox limpo 481 ms / 67 MB. Detalhes em `docs/fase0/s5-baseline.md`. CPU segue provisória até a Fase 4.

| Métrica | Meta | Como medir | Fase |
|---|---|---|---|
| Abertura a frio até a janela principal visível | < 1 s | timestamps no log | 1, 6 |
| RAM só na bandeja (sem janelas) | < 30 MB (era < 40; base medida 1,4 MB) | soma do Private Working Set do app + processos `msedgewebview2` filhos | 1 |
| RAM com a janela principal aberta | < 120 MB (era < 150; base medida 70 MB) | idem | 1 |
| Abrir a mini ao começar a gravar | < 600 ms (base medida ~400–500 ms por janela) | timestamps no log | 1 |
| RAM gravando (modelo + mini) | < 1,5 GB | idem | 4 |
| CPU média gravando (4 núcleos / 8 threads) | < 20% | `typeperf` por 10 min | 4 |
| Atraso do fim da fala até o texto na mini | p50 ≤ 3 s · p95 ≤ 6 s | timestamps no log | 4 |
| Tamanho do instalador (sem modelo) | < 25 MB | tamanho do arquivo | 1, 6 |
| Texto perdido ao matar o processo | no máximo o segmento em andamento | teste manual | 5 |
| Fator de tempo real (arquivo) | informado (duração ÷ tempo) | relatório | 3 |
| Carga do modelo | informada | relatório | 0, 3 |

---

## 5. Stack (ajustada)

- **Shell:** Tauri v2 (Rust stable, MSVC). Plugins: `single-instance`, `global-shortcut`, `dialog`, `fs`, `log`, `clipboard-manager` (copiar transcrição), `opener` (abrir o navegador e pastas). Bandeja pelo recurso nativo `tray-icon`. **Sai o `shell`** (L13).
- **Frontend:** Svelte 5 + Vite + TypeScript, com CSS próprio baseado em tokens.
- **Áudio:** crate `wasapi` (decidido no S3, ADR-002) + crate `windows` para MMCSS, formato do endpoint e sessões de áudio. O `autoconvert` do WASAPI reamostra para 16 kHz mono float (decidido na Fase 4, no lugar do `rubato`) e um `std::sync::mpsc` liga a captura ao pipeline (no lugar do `ringbuf`).
- **ASR/VAD:** crate oficial `sherpa-onnx` com o Parakeet TDT 0.6b v3 int8 e o `silero_vad.onnx`.
- **Rede:** `ureq` 3 (síncrono, uma thread de download) com `rustls` + `rustls-platform-verifier` (certificados do Windows, **nunca** só `webpki-roots`) e proxy do sistema (`win-system-proxy`) mais proxy manual; PAC via WinHTTP se aparecer. SHA-256 pelo BCrypt do Windows (sem `sha2`). Decidido no S4 (`docs/fase0/s4-download.md`). `bzip2` + `tar` só se o pacote vier compactado (o espelho do HF serve os arquivos soltos).
- **Banco:** `rusqlite` com `bundled` (FTS5), WAL e `synchronous=NORMAL`.
- **Diff de palavras:** `similar`.
- **FFmpeg:** o do PATH ou baixado sob demanda. Só é usado na importação de arquivo e na Parte 5, para seguir o prompt.txt (`silencedetect`). Fonte do download (decidido na Fase 2): build estática *essentials* da gyan.dev, release fixa `GyanD/codexffmpeg` 9.0.2 (`.zip`, 109 MB, GPL-3.0, SHA-256 no catálogo); só o `ffmpeg.exe` e a licença vão para `bin\`, extraídos com o crate `zip` (só `deflate`, Rust puro).

---

## 6. Arquitetura revisada

```
┌──────── Frontend (Svelte 5, WebView2) · janelas criadas sob demanda ────────┐
│ main  : Sessões · Busca · Detalhe · Importar · Legendas/EDL · Config · Modelos│
│ mini  : ao vivo EU/OUTROS · medidor por canal · ●REC · ⏸ · ■ · ⭐ marcar     │
│ wizard: boas-vindas · privacidade · modelo · teste de áudio · atalhos        │
└──────────────────────── invoke() / events ───────────────────────────────────┘
┌──────── src-tauri (app) ────────────────────────────────────────────────────┐
│ commands/ camada fina sobre o core                                           │
│ shell/    stealth · bandeja · atalhos · instância única · janelas sob demanda│
│ diag/     exportar diagnóstico (sem texto nem áudio)                         │
└──────────────────────────────────────────────────────────────────────────────┘
┌──────── crates/ouveai-audio (Windows) ──────┐ ┌──────── crates/ouveai-cli ───────┐
│ AudioSource: Mic · LoopbackEndpoint          │ │ transcribe [--check]             │
│ (multimídia + comunicação) · LoopbackProcesso│ │ transcribe --from-json           │
│ (Win11, opcional) · relógio QPC · MMCSS      │ │ replay eu.wav outros.wav  (novo) │
│ troca de dispositivo · formato do endpoint   │ │ compare (P4) · edl-subs (P5)     │
└──────────────────────────────────────────────┘ └──────────────────────────────────┘
┌──────── crates/ouveai-core (sem Tauri, sem WASAPI) ─────────────────────────┐
│ asr/  words/  subs/  media/  compare/  edl/  store/  models/                 │
│ net/   → downloader (certificados do SO, proxy, retomada, hash)              │
│ live/  → pipeline por canal: RMS → Silero → fila → Parakeet → palavras →     │
│          alucinação → eco → store · fonte Replay (WAV) para testes           │
│ paths/ → diretório local único para o app e a CLI                            │
└──────────────────────────────────────────────────────────────────────────────┘
```

### 6.1 Por que o `live/` foi para o core
O pipeline ao vivo (VAD → ASR → palavras → eco → banco) não depende de WASAPI nem do Tauri: ele recebe amostras de um `AudioSource`. Com isso, `cargo test` roda o pipeline inteiro alimentado por dois WAVs (replay), e a CLI ganha `ouveai replay` para reproduzir um problema com o áudio salvo de um usuário. A captura do Windows fica isolada em `ouveai-audio`, o que também prepara o macOS.

### 6.2 Janelas sob demanda
- **Na bandeja:** nenhuma WebView aberta, então a RAM é mínima.
- **Janela principal:** criada quando o usuário abre e **destruída** quando fecha (o app continua na bandeja).
- **Mini:** criada ao começar a gravar e destruída ao parar.
- **Troca:** abrir uma janela custa algumas centenas de ms. Se a Fase 1 mostrar que é perceptível, a mini é pré-criada oculta, e essa decisão sai das medições.

### 6.3 Relógio da sessão
Todos os tempos ao vivo são relativos ao início da sessão. Cada pacote de áudio é posicionado pelo seu QPC, não pela contagem de amostras, porque o loopback não entrega pacotes durante o silêncio. As lacunas são preenchidas com silêncio. Os relógios dos dois dispositivos derivam um pouco, mas isso só pesa para ordenar as falas, e o QPC resolve.

### 6.4 Fluxo de uma fala ao vivo
```
captura (thread MMCSS, autoconvert 16 kHz mono) → mpsc → Silero VAD
  → segmento fecha (~800 ms de silêncio; fala corrida: 1ª pausa de 120 ms depois de 10 s, ou aos 20 s no ponto de menor energia dos últimos 3 s)
  → fila única (reunião tem prioridade sobre importação)
  → Parakeet (2 threads ao vivo) → core::words (+ offset da sessão)
  → filtro de alucinação → eco (marca, não apaga)
  → INSERT no SQLite (por segmento) → evento `segment` → mini e principal
```

### 6.5 Processo único (decisão consciente)
O ASR roda no mesmo processo, que é mais simples e gasta menos RAM. O risco é que uma falha nativa no onnxruntime derrube o app no meio da reunião (R16). A mitigação é gravar cada segmento assim que ele fica pronto e recuperar sessões órfãs. **Gatilho para mudar:** se o piloto mostrar falhas nativas, o reconhecedor vai para um processo trabalhador separado, e o core já está desenhado para isso.

### 6.6 Diretórios de dados
Ficam no diretório local do app (`app_local_data_dir`, dentro de `%LOCALAPPDATA%`). O mesmo caminho é resolvido por `core::paths`, para a CLI enxergar os mesmos modelos:
```
ouveai.db  ·  models\  ·  bin\ffmpeg.exe  ·  recordings\<sessão>\eu.wav|outros.wav  ·  logs\
```
Nada vai para o Roaming. O WebView2 guarda o perfil dele na mesma pasta local.

### 6.7 O que sai da máquina (lista fechada)
1. O download do modelo e do FFmpeg, sempre confirmado.
2. A verificação de atualização, só se você escolher essa opção em **(D5)**, e manual.

Fora disso, nenhuma chamada de rede.

---

## 7. Regras centrais (valem para todo o código)

1. **Tempo por palavra sempre medido** (prompt.txt): vem dos timestamps/durations do modelo ou da análise do áudio (Silero VAD ao vivo, `silencedetect` do FFmpeg em arquivo). Nunca dividir frase, interpolar nem ajustar à mão.
2. **Montagem das palavras** (`core::words`): token com `▁` (ou espaço, conforme a versão instalada) abre palavra nova, token sem marcador gruda na anterior, e o 1º token sempre abre palavra. Início = timestamp do 1º token. Fim = início + duração do último token **que não seja só pontuação** (no Parakeet a pontuação tem timestamp próprio, depois da pausa — achado do S1), quando há `durations`. Sem `durations`: fim = início da próxima palavra, recuado ao começo do silêncio medido entre as duas, limitado a início + 1,5 s. A última palavra fica limitada à duração do áudio.
3. **Pausas** = silêncio medido (`silencedetect -35 dB, 0,3 s` em arquivo, segmentos do VAD ao vivo), nunca a diferença entre inícios.
4. **Segmentação de legenda única** (`core::subs::segment`) com a configuração no topo: `MAX_CHARS=36, PAUSA_QUEBRA=0.45, VIRGULA_MIN_CHARS=20, SAIDA_APOS_FIM=0.1, MIN_DURACAO=1.0, INTERVALO_MIN=0.08`, com as regras de quebra do prompt.txt.
5. **Não inventar API:** ler o README/docs.rs da versão instalada e o exemplo oficial `nemo_parakeet.rs`, e usar os nomes de campo exatos.
6. **O áudio nunca sai da máquina** (seção 6.7).
7. **Nenhuma heurística apaga fala.** Eco suspeito é marcado e pode ser recuperado. Só se descarta ruído evidente (texto vazio, repetição cíclica).
8. **Logs sem conteúdo:** nunca gravar texto transcrito nem título de sessão no log.
9. **Dados só em `%LOCALAPPDATA%`.**
10. **Mexeu em dependência nativa ou no instalador? Testa na máquina limpa** (Windows Sandbox) antes de dar a fase como pronta.

---

## 8. Fases de execução

> **Regras para o agente executor** (mantidas do PLANO.md): **parar ao fim de cada fase** com um resumo ✅ e esperar o "ok". **Pedir confirmação antes de cada instalação ou download**, dizendo o que é, de onde vem e o tamanho. Nada global nem como administrador sem autorização. Se algo falhar, mostrar o erro e propor a correção, **sem trocar biblioteca, modelo ou abordagem sem perguntar**. Responder sempre em pt-BR. **Novo:** se uma fase depende de uma decisão D pendente, perguntar antes de implementar a parte afetada.

**De → para (numeração)**

| PLANO.md | Plano revisado | Principal mudança |
|---|---|---|
| F0 Diagnóstico e spike | **F0** Diagnóstico e spikes | 5 spikes e decisão Go/No-go |
| F1 Esqueleto | **F1** Esqueleto + instalador mínimo | Máquina limpa, janelas sob demanda, atalhos novos, `%LOCALAPPDATA%` |
| F2 Modelos e FFmpeg | **F2** Modelos, FFmpeg e primeiro uso | Retomada, proxy, navegador e arquivo local |
| F3 Arquivo | **F3** Arquivo + banco mínimo | Store antecipado, heurísticas de "suspeitos" |
| F4 Ao vivo | **F4** Ao vivo | AudioSource, QPC, Bluetooth, eco marcado, medidores |
| F5 Persistência | **F5** Histórico, busca e utilidades | Copiar Markdown, ⭐, retenção |
| F8 Empacotamento | **F6** Distribuição e piloto | Antecipada, com piloto e critérios de saída |
| F6 Whisper | **F7** Parte 4 | Igual, depois da v1.0 |
| F7 EDL | **F8** Parte 5 | Igual, depois da v1.0 |

Tamanho relativo: **P** (pequeno) · **M** (médio) · **G** (grande).

---

### Fase 0: Diagnóstico e spikes técnicos · M · Go/No-go

**Objetivo:** eliminar, antes de escrever o app, os riscos que podem mudar a arquitetura.

**Diagnóstico**
- **Máquina:** SO e build, CPU (modelo, núcleos, AVX2), RAM e disco livre (~2 GB para o Parakeet, +3 GB para a Parte 4).
- **Ferramentas:** Rust/MSVC, Node LTS, WebView2 e FFmpeg no PATH.
- **Windows Sandbox:** verificar se está disponível. Habilitar exige admin, então pedir autorização. A alternativa é uma VM limpa.
- **Repositório:** `git init` e `.gitignore` (models, target, node_modules, recordings).

**Spikes** (em `crates/*/examples/`)
- **S1 · ASR** (os itens 1, 2 e 5 do PLANO). Carregar o Parakeet v3 int8 pelo crate `sherpa-onnx` (download confirmado, ~640 MB) e transcrever um WAV curto em pt-BR. Relatar:
  - os campos reais do resultado (`tokens`, `timestamps`, `durations`?) e o marcador de palavra;
  - o tempo de carga, o fator de tempo real (definição do prompt) e a RAM de pico;
  - se hotwords / `modified_beam_search` funcionam com TDT (só relatar).

  **Se não houver timestamps, parar** e trazer as opções (por exemplo, a C API via FFI).
- **S2 · Linkagem nativa e máquina limpa** (o item 3 do PLANO, ampliado). Listar as dependências de cada binário (`dumpbin /dependents`) e comparar três caminhos:
  - **(A)** sherpa e onnxruntime linkados de forma estática num exe só;
  - **(B)** DLLs ao lado do exe, com as DLLs do runtime C++ junto (implantação local);
  - **(C)** DLLs com delay-load e checagem por `LoadLibraryExW` antes do 1º uso, para o app abrir e mostrar um erro amigável.

  Rodar o exe do spike no **Windows Sandbox sem VC++ Redist**. Critério: abre na máquina limpa e, se faltar algo, quem fala é o app, não o Windows.
- **S3 · Captura** (o item 4 do PLANO, ampliado). Mic e loopback ao mesmo tempo, gravando 2 WAVs, nestes cenários:
  - **(a)** padrão multimídia diferente do padrão de comunicação, com Meet no Chrome e Teams desktop;
  - **(b)** fone Bluetooth entrando numa chamada (troca para HFP), para ver se OUTROS continua;
  - **(c)** 30 s sem nada tocando, para ver se há lacunas de pacotes e se o relógio QPC fecha;
  - **(d)** desconectar o fone com a captura rodando;
  - **(e)** loopback por processo (Win11) com Teams e Chrome, só como prova de conceito.

  Decidir entre `wasapi` e `cpal`.
- **S4 · Download corporativo.** Baixar o modelo com o cliente HTTP escolhido, interromper e retomar (`Range`). Testar **na rede da empresa**, se possível no notebook corporativo e atrás do proxy. Calcular o SHA-256 e o tamanho para o catálogo e verificar se existe espelho alternativo.
- **S5 · Linha de base de desempenho.** Com um app Tauri vazio, medir a abertura a frio e a RAM com 0, 1 e 2 janelas (somando os processos do WebView2), e calibrar a seção 4.

**Entregas:** relatório; `docs/adr/001-stack-asr.md`, `002-captura-audio.md`, `003-empacotamento-nativo.md`; catálogo de modelos com hash.

**Go quando:**
- S1 tem timestamps (ou você aceitou o plano B);
- S2 abre na máquina limpa;
- S3 capta OUTROS nos cenários (a) a (d).

Se não, parar e decidir com você.

---

### Fase 1: Esqueleto do app + instalador mínimo · M

**Estrutura**
- Workspace Cargo (`ouveai-core`, `ouveai-audio`, `ouveai-cli`, `src-tauri`) e Svelte 5 + Vite + TS.
- `core::paths` aponta para `%LOCALAPPDATA%`.

**Janelas sob demanda (6.2)**
- `main`: normal.
- `mini`: ≈360×220, sem moldura, `always_on_top`, `skip_taskbar`, arrastável.
- A posição é salva e validada contra os monitores atuais (L19).
- Avaliar a mini sem pegar foco (L20).

**Stealth**
- Comando `set_stealth(on)` aplica `set_content_protected` nas janelas abertas e nas que forem criadas depois.
- Ligado por padrão.
- Build < 19041: começa desligado e o app avisa.

**Bandeja, atalhos e instância única**
- Menu da bandeja: Abrir · Mostrar mini · Gravar/Parar · Stealth on/off · Configurações · Sair.
- Atalhos globais padrão (configuráveis). Se o registro falhar, avisar e sugerir outro:

  | Ação | Padrão |
  |---|---|
  | Mostrar/ocultar a mini (centraliza se estiver fora da tela) | `Ctrl+Alt+Shift+O` |
  | Gravar / parar | `Ctrl+Alt+Shift+R` |
  | Marcar momento ⭐ | `Ctrl+Alt+Shift+M` |

  Evitados de propósito: Teams `Ctrl+Shift+M/O/E/K`, Zoom `Alt+A/V/S/R`, Meet `Ctrl+D/E`, Windows `Alt+Shift`. **(D3)**
- Instância única: a segunda instância foca a primeira.

**Logs e configurações**
- Logs em arquivo, com rotação e sem conteúdo (regra 8).
- Tela básica de Configurações, gravando na tabela `settings`.

**Instalador mínimo**
- NSIS por usuário, sem admin, com a estratégia do ADR-003.
- Instalado e aberto no Windows Sandbox.

**Pronto quando:**
- o app abre em < 1 s e a RAM fica dentro das metas calibradas;
- as janelas **não aparecem** no print (Ferramenta de Captura), no OBS nem no compartilhamento do Teams/Meet, mas aparecem no monitor;
- **no Teams, `Ctrl+Shift+O` continua ligando a câmera**;
- o instalador roda no Sandbox limpo.

---

### Fase 2: Modelos, FFmpeg e primeiro uso · M

**Catálogo (`core::models`)**
- Cada modelo tem id, versão, URL principal, espelho, tamanho, SHA-256 e tamanho extraído.
- Estados: ausente · baixando · pausado · verificando · extraindo · pronto · corrompido.

**Downloader (`core::net`)**
- Valida pelos certificados do Windows; usa o proxy do sistema ou o campo manual nas Configurações.
- Grava em `.part`, retoma por `Range` e tenta de novo com espera crescente.
- Mostra progresso por evento (%, velocidade, tempo restante).
- Confere tamanho e SHA-256.
- Extrai o `.tar.bz2` numa pasta temporária, faz o rename atômico da pasta e apaga o pacote.
- Checa disco e RAM antes de começar.

**Plano B sempre visível na tela de modelos**
- **"Baixar pelo navegador":** abre a URL no navegador padrão, que já resolve o proxy e o login da empresa.
- **"Importar arquivo ou pasta":** o `.tar.bz2` ou a pasta extraída, de pendrive ou de pasta de rede, com a mesma conferência de hash.

**FFmpeg**
- Usa o do PATH. Se não houver, oferece a build estática para o diretório local, pelo mesmo downloader e com confirmação.
- **Só é pedido no primeiro "Importar arquivo"**, porque a reunião não precisa de FFmpeg.
- Roda sem janela de console.

**Assistente de primeiro uso**
- Boas-vindas → privacidade (lembrete de LGPD) → checagem da máquina → baixar ou importar o modelo → atalhos e bandeja.
- O teste de áudio entra na Fase 4.

**Pronto quando:**
- um download interrompido retoma sem corromper e um arquivo adulterado é recusado;
- a importação de arquivo funciona sem internet;
- a UI mostra o estado de cada modelo;
- o download foi testado atrás do proxy da empresa, ou o plano B foi validado lá.

---

### Fase 3: Núcleo de transcrição de arquivo (Partes 2 e 3) + banco mínimo · G

**Mídia e reconhecimento**
- `core::media`: FFmpeg gera um WAV temporário 16 kHz mono PCM16 (apagado no fim) e roda o `silencedetect` (-35 dB, 0,3 s).
- **Áudio longo:** acima de 60 s, cortar num silêncio entre 45 s e 60 s (no meio do maior silêncio da janela). Sem silêncio, cortar no ponto de menor volume e **avisar** (acima de 3 cortes forçados, um aviso-resumo). Somar o offset de cada pedaço. *(Decidido na Fase 3: o prompt.txt pede 10 min, mas o encoder do Parakeet ONNX aborta acima de 400 s e, em trechos longos com ruído de fundo, pula frases inteiras. Medido com um podcast de 65 min com legenda de referência e uma gravação ruidosa: 60 s empata no áudio limpo, recupera as frases no ruidoso e é 1,6× mais rápido que 5 min. O `Asr` ainda recusa trechos acima de 390 s.)*
- `core::asr`: reconhecedor offline `nemo_transducer`, em CPU, com `threads = min(4, núcleos)` para arquivo.
- `core::words` e `core::subs` seguem as regras da seção 7.

**Saídas e CLI**
- Arquivos UTF-8 ao lado da entrada: `.json` `{"source","duration","words":[{word,start,end}],"pauses":[{start,end}]}` com 3 casas, mais `.txt` e `.srt`.
- `--check` gera só o `.check.json`; `--from-json` regenera `.txt`/`.srt`.
- CLI `ouveai transcribe`.

**UI**
- Tela "Importar arquivo": arrastar e soltar, progresso, abrir/exportar.
- Editor que corrige só `word`, nunca `start`/`end`, e regenera.

**Banco mínimo (`core::store`)**
- Migrações por `PRAGMA user_version`.
- Tabelas `sessions`, `segments`, `words`, `settings`.
- Recuperação de sessões órfãs.
- O arquivo importado vira uma sessão `kind='file'`.

**Fila do reconhecedor:** a reunião tem prioridade, e uma importação durante a reunião espera, com aviso (L18).

**Relatório no fim**
- Tempo de carga, tempo de transcrição e fator de tempo real (duração ÷ tempo).
- **Candidatos a erro**, com tempo e taxa por minuto (acima de 3/min, sugerir a Parte 4). Heurísticas explícitas:
  1. grafias de português europeu com equivalente pt-BR (equipa→equipe, facto→fato, registo→registro, utilizador→usuário, ecrã→tela, telemóvel→celular, …);
  2. palavras com inicial maiúscula fora do início de frase (prováveis nomes próprios);
  3. tokens de baixa confiança, se a API expuser essa informação (S1: o wrapper Rust não expõe; avaliar via `sherpa-onnx-sys`);
  4. palavras curtas de inglês/espanhol no meio de texto em português (but, and, or, the, of, for, reunion, …): o Parakeet v3 não fixa o idioma e troca palavras curtas (achado do S1).

**Testes unitários**
- `words`: tokens sintéticos com e sem `durations`, pontuação grudando, 1º token.
- `subs`: cada regra de quebra, `MIN_DURACAO`, `INTERVALO_MIN`.
- `store`: migração e sessão órfã.

**Pronto quando:**
- um vídeo de 1–2 min gera os 3 arquivos e a legenda sincroniza no VLC;
- o fator de tempo real é informado;
- a sessão aparece no banco.

---

### Fase 4: Captura ao vivo e transcrição de reunião · G · maior risco

**Fontes de áudio (`ouveai-audio`)**
- Trait `AudioSource` com as implementações:
  - `Mic`;
  - `LoopbackEndpoint`: padrão multimídia mais padrão de comunicação, sem duplicar e somados;
  - `LoopbackProcesso`: opcional, Win11 **(D7)**;
  - `Replay`: no core, a partir de dois WAVs.
- Conversão para mono e 16 kHz pelo `autoconvert` do WASAPI e `std::sync::mpsc` sem bloquear a captura (decidido na Fase 4, sem dependência nova), threads de captura com MMCSS.
- Relógio da sessão por QPC, com as lacunas preenchidas (6.3).

**Dispositivos**
- Acompanhar a troca de padrão (`IMMNotificationClient`), tratar `AUDCLNT_E_DEVICE_INVALIDATED` e reabrir sem encerrar a sessão.
- Cada troca gera um marcador automático "troca de dispositivo" e um aviso na UI.
- Seletor nas Configurações: **Automático (recomendado)** · Dispositivo específico · Por aplicativo (se D7).
- Permissão do mic lida no registro (`winreg`), fora da thread principal, com cache de 1 s.
- **Antivírus/EDR segurando a captura (achado do S3):** com o Kaspersky, a abertura do mic ou do loopback (`IAudioClient::Initialize`) fica bloqueada até o usuário aprovar o aviso do antivírus, de 3 a 19 s no teste. Cada build novo do exe pede de novo. Consequências:
  - cada fonte abre na sua própria thread, e EU nunca espera por OUTROS;
  - se uma fonte não abrir em ~2 s, a mini mostra "aguardando permissão (antivírus?)" no canal;
  - se não abrir em ~60 s, avisa e oferece tentar de novo;
  - o guia rápido explica o aviso do antivírus.
- O mic só abre quando a gravação começa.
- **Qual mic é o EU (achado do S3):** o "dispositivo de comunicação padrão" pode não ser o mic onde a pessoa fala. Nesta máquina ele é a webcam C920, e o usuário fala no USB PnP, que é o "dispositivo padrão". Regras:
  - o modo Automático usa o **dispositivo padrão** (papel Console);
  - durante a gravação, o app olha as sessões de captura ativas (`IAudioSessionManager2`). Se o app de reunião estiver captando de **outro** mic, avisa e oferece trocar;
  - o medidor do EU e o "Testar áudio" mostram qual mic está sendo usado;
  - **nunca abrir sozinho o mic de um fone Bluetooth** (form factor Headset/Handset) que nenhum app esteja usando. Abrir esse mic força o perfil de chamada (HFP) e deixa o som "abafado" durante a reunião inteira. Nesse caso, esperar o app de reunião abrir o mic ou perguntar;
  - o posicionamento pelo QPC usa **tolerância de ~200 ms** no mic, porque o mic BT em HFP tem jitter. Com 10 ms a faixa picota e a transcrição se perde (S3, f1 × f2).

**Pipeline (`core::live`), por canal**
- RMS → Silero VAD → segmento (fecha com ~800 ms de silêncio; fala corrida: fecha na 1ª pausa curta, de 120 ms, depois de 10 s; sem pausa, corta aos 20 s no ponto de menor energia dos **últimos 3 s**, L17. Cortar mais atrás (entre 12 e 20 s) levava o p95 do atraso a 10 s) → fila → Parakeet → `core::words` (offset da sessão) → filtro de alucinação → eco → banco (a cada segmento) → evento `segment`.
- Um só reconhecedor compartilhado, com **2 threads ao vivo** (configurável).
- A gravação **começa na hora**: enquanto o modelo carrega, os segmentos esperam na fila (L10).
- O idle TTL (5 min, configurável) só vale fora da gravação.

**Eco**
- O tipo do dispositivo de saída define o padrão de "Estou usando fone", com ajuste manual.
- **Com fone:** nenhum filtro.
- **Sem fone:** correlação de energia mic × loopback + sobreposição no tempo + similaridade de texto → `eco_suspeito`, oculto por padrão e visível com um clique.
- A UI recomenda usar fone.

**Mini janela**
```
┌────────────────────────────────────── ● 00:12:41 ┐
│ OUTROS ▮▮▮▯▯            EU ▮▯▯▯▯                 │
│ OUTROS: …então a gente fecha o escopo na sexta.  │
│ EU: Fechado, mando o resumo hoje.                │
│ [⏸] [■] [⭐ Marcar]                         [👁] │
└──────────────────────────────────────────────────┘
```
- Se OUTROS ficar sem sinal por ~60 s com a sessão ativa, aparece a dica "Não estou ouvindo os outros participantes", com um atalho para o seletor de dispositivo.

**Outros itens**
- **"Testar áudio"** (Configurações e assistente): falar (EU) e tocar um som (OUTROS), com os medidores.
- Opção **salvar áudio da reunião** (WAV por canal), seguindo a retenção **(D2)**.
- Pausar, retomar e parar.

**Testes**
- Replay determinístico: EU + OUTROS, eco simulado, perda de dispositivo injetada.
- Unitários do eco e do corte forçado.

**Pronto quando**, em chamadas reais de 10 min no **Teams desktop, no Meet pelo Chrome e no Zoom** (se disponível):
- as falas aparecem rotuladas EU/OUTROS na mini, com atraso dentro da meta, e tudo fica salvo;
- desconectar o fone e trocar o perfil do Bluetooth no meio **não derruba a sessão nem perde OUTROS**;
- a CPU média fica dentro da meta;
- sem fone, os ecos ficam marcados e a fala real do EU é preservada.

---

### Fase 5: Histórico, busca e utilidades de reunião · M

> **Feita em 01/11/2026** ([resultado](fase5/resultado.md)). Busca e cópia ficam em Sessões; "Excluir tudo" fica em Configurações › Dados. A limpeza do áudio roda de hora em hora (mais estrita que "uma vez por dia"). Achou e corrigiu dois defeitos da captura com dispositivos de relógio inválido (Áudio Remoto/RDP). **Fases 4 e 5 validadas pelo usuário em chamadas reais (Teams, Meet, Zoom, fone e Bluetooth) em 01/11/2026.**

**Schema completo (seção 9):** `marks`, `flags`, `speaker_label`, FTS5 com triggers.

**UI principal**
- Lista de sessões e busca com destaque, sem diferenciar acento.
- Detalhe com a linha do tempo EU/OUTROS e os marcadores. Clicar numa fala toca o áudio, se houver.
- Renomear, excluir e **"Excluir tudo"**.

**Exportar e copiar**
- Exportar `.txt/.srt/.json/.md`.
- **Copiar transcrição** (Markdown), no formato `**[00:12:03] OUTROS:** texto`:
  - falas seguidas do mesmo canal ficam agrupadas;
  - os marcadores ⭐ entram;
  - os ecos suspeitos ficam de fora;
  - o tempo é relativo ao início, com opção de hora do relógio.

**Retenção do áudio:** limpeza automática ao iniciar e uma vez por dia, com o uso de disco visível nas Configurações.

**Pronto quando:**
- matar o processo durante a gravação preserva o texto até o último segmento;
- a busca acha palavras sem acento;
- o texto copiado cola formatado no chat do Teams;
- o áudio vencido é apagado.

---

### Fase 6: Distribuição e piloto · M (era a Fase 8, antecipada)

> **Parte técnica feita em 01/11/2026** ([resultado](fase6/resultado.md), [ADR-005](adr/005-distribuicao-e-atualizacao.md)): instalador 0.9.0, Sobre com licenças, diagnóstico e verificação manual de atualização, guia rápido, mensagem ao TI e formulário. Falta o piloto em si, que depende do retorno do TI. O repositório virou público e a release v0.9.0 saiu em 01/11/2026. A **0.9.1** (02/11/2026) leva o ícone definitivo, as Fases 7 e 8 e o aviso de WebView2. A **0.9.2** (02/11/2026) troca o modelo de fala pelo ajuste pt-BR do Parakeet ([ADR-009](adr/009-parakeet-ptbr.md)), melhora a comparação com o Whisper e descarta o Nemotron ([ADR-008](ADR-008-substituicao-whisper-por-nemotron.md)); é a versão que vai para o TI e o piloto.

**Instalador e app**
- NSIS por usuário, com a estratégia do ADR-003.
- WebView2 em modo bootstrapper (padrão, e a maioria das máquinas já tem). Avaliar embutir o bootstrapper se a rede for restrita.
- Checagem de SO mínimo.
- O desinstalador pergunta se apaga os dados.
- A CLI não vai no instalador.

**Tela "Sobre":** versão, licenças e atribuições (Parakeet CC-BY-4.0, sherpa-onnx Apache-2.0, onnxruntime MIT, Silero VAD MIT, FFmpeg LGPL/GPL conforme a build).

**Exportar diagnóstico (.zip):** versão, SO e build, CPU e flags, RAM, dispositivos de áudio, configurações e logs. Sem texto, sem áudio, sem títulos.

**Atualizações e distribuição**
- Atualizações conforme **(D5)**.
- Distribuição conforme **(D4)**: falar com o TI antes (EDR, SmartScreen) e publicar o SHA-256 do instalador.

**Documentação:** guia rápido de 1 página (instalar, baixar ou importar o modelo, primeira reunião, atalhos, fone e eco, privacidade) e README em pt-BR.

**Medições finais:** todas as da seção 4.

**Piloto**
- 3–5 pessoas, ~2 semanas, em máquinas corporativas reais.
- Formulário curto depois de cada reunião e triagem semanal.
- A Fase 7 pode começar durante o piloto, se você quiser.

**Pronto (v1.0) quando o piloto cumprir os critérios de saída:**
- a maioria instalou sem ajuda remota;
- nenhuma perda de texto;
- OUTROS foi captado no Teams, no Meet e no Zoom (ou a dica resolveu);
- nenhum bloqueio de antivírus/EDR em aberto;
- zero bugs críticos.

---

### Fase 7: Parte 4, comparação com Whisper (opcional, sob pedido) · M

- **Antes de instalar, propor a forma e esperar a escolha.** Proposta padrão: Whisper **large-v3** (~3 GB, como o prompt pede) em ONNX pelo próprio sherpa-onnx, sem Python, `language="pt"`, com cortes de até 30 s em silêncio medido. Apresentar o **large-v3-turbo** como alternativa mais rápida, se estiver no catálogo do sherpa-onnx.
- `core::compare`: rodar os dois modelos no mesmo áudio, alinhar as palavras por sequência (`similar`, texto normalizado) e mostrar só os trechos divergentes, com o tempo. O resultado final usa o texto que você escolher e **os tempos do Parakeet**.
- Em reunião, só funciona com o áudio salvo.
- UI lado a lado com escolha por trecho, e CLI `ouveai compare`.

> **Feito em 01/11/2026.** Escolhas: **large-v3-turbo int8** e crate `similar`. Janelas de 20 s, formas coloquiais e muletas fora da lista, janelas em que o Whisper falha descartadas. Ver [ADR-006](adr/006-comparacao-whisper.md) e [resultado](fase7/resultado.md).

### Fase 8: Parte 5, legenda de vídeo editado (EDL) · M

- `core::edl`: lê o `edl.json` (`file, in, out, position`) e valida antes de tudo:
  - `out > in`;
  - sobreposição no vídeo final é erro;
  - lacunas entre cortes são permitidas e listadas uma vez.

  O `.json` de mesmo nome-base é obrigatório, com erro claro se faltar.
- Uma palavra entra se `start ∈ [in, out)`. Tempo final = tempo no bruto − in + position, e um fim que passa de `out` é cortado em `out`. Usa a mesma `segment()`, e nenhum bloco atravessa um corte.
- **Verificação:**
  - rodar `transcribe --check` no vídeo final;
  - alinhar por sequência com texto normalizado (minúsculas, sem pontuação nem acento), contando só os pares casados;
  - informar o desvio mediano e o máximo dos inícios e as palavras sem par (referência: 0,12 s);
  - listar desvios > 0,3 s com a causa provável e propor a correção **na origem** (`edl.json`), sem empurrar tempos.
- UI "Legendas" e CLI `ouveai edl-subs`.

> **Feito em 02/11/2026.** A correção proposta só é aplicada com um clique, com cópia em `edl.json.bak`. A legenda leva o nome do vídeo final quando ele é verificado. Ver [ADR-007](adr/007-legenda-edl.md) e [resultado](fase8/resultado.md).

---

## 9. Modelo de dados (revisado)

```sql
-- PRAGMA foreign_keys=ON; PRAGMA journal_mode=WAL; PRAGMA synchronous=NORMAL;
-- migrações versionadas por PRAGMA user_version

sessions(
  id INTEGER PRIMARY KEY, title TEXT,
  kind TEXT CHECK (kind IN ('meeting','file')),
  source_path TEXT NULL, started_at TEXT, ended_at TEXT NULL,
  status TEXT CHECK (status IN ('recording','paused','ended','interrupted')),
  duration_s REAL, audio_dir TEXT NULL, audio_expires_at TEXT NULL,
  app_version TEXT,
  pauses TEXT NULL)  -- JSON das pausas medidas (arquivo), para refazer o .srt depois de corrigir (Fase 3)

segments(
  id INTEGER PRIMARY KEY,
  session_id INTEGER REFERENCES sessions(id) ON DELETE CASCADE,
  channel TEXT CHECK (channel IN ('EU','OUTROS','ARQUIVO')),
  speaker_label TEXT NULL,          -- futuro: diarização ("OUTROS 1")
  start_s REAL, end_s REAL, text TEXT,
  flags INTEGER DEFAULT 0)          -- bit 0 = eco_suspeito · bit 1 = editado

words(
  id INTEGER PRIMARY KEY,
  segment_id INTEGER REFERENCES segments(id) ON DELETE CASCADE,
  idx INTEGER, word TEXT, start_s REAL, end_s REAL)

marks(
  id INTEGER PRIMARY KEY,
  session_id INTEGER REFERENCES sessions(id) ON DELETE CASCADE,
  t_s REAL, kind TEXT CHECK (kind IN ('estrela','dispositivo','pausa')),
  note TEXT NULL)

settings(key TEXT PRIMARY KEY, value TEXT)   -- valor em JSON

-- segments_fts: FTS5 content=segments, tokenize='unicode61 remove_diacritics 2'
-- sincronizada por triggers ai/ad/au
```

- **Com WAL + `synchronous=NORMAL`,** um crash do app não perde transações confirmadas (só uma queda de energia pode perder a última). Isso basta para a meta "matar o processo preserva o texto".
- **Sessões órfãs no boot:** `recording`/`paused` viram `interrupted`, com o texto preservado.

---

## 10. Estratégia de testes e verificação

| Camada | O quê | Onde roda |
|---|---|---|
| Unitário (core) | `words`, `subs`, `edl`, `compare`, eco (correlação), corte forçado, migrações | `cargo test -p ouveai-core` (CI e local) |
| Golden | 2–3 WAVs curtos em pt-BR com transcrição de referência, para pegar regressões de texto e tempo | Local (`--features modelo`), já que o modelo não vai para o CI |
| Replay | Pipeline ao vivo completo com `eu.wav` + `outros.wav`, incluindo eco simulado e perda de dispositivo injetada | Local / `ouveai replay` |
| Máquina limpa | Instalar e abrir no Windows Sandbox | A cada fase que mexe em dependência nativa ou no instalador |
| Manual E2E | Checklist abaixo | Fim das Fases 4, 5 e 6 |
| CI (opcional, barato) | GitHub Actions `windows-latest`: `fmt`, `clippy`, testes do core, `svelte-check` e o instalador como artefato | A cada push |

**Checklist fim a fim**
1. CLI: `ouveai transcribe teste.mp4` gera `.json/.txt/.srt` sincronizados no VLC; `--from-json` depois de editar uma palavra; `--check` não toca no `.srt`.
2. Reunião de 10 min no Teams desktop, no Meet (Chrome) e no Zoom, com fone com fio, fone Bluetooth e sem fone.
3. Trocar de dispositivo e de perfil Bluetooth no meio da reunião.
4. Stealth: compartilhar a tela inteira no Teams/Meet e gravar no OBS. As janelas não aparecem na transmissão, mas aparecem no monitor.
5. Atalhos: os do OuveAI funcionam, e `Ctrl+Shift+O` / `Ctrl+Shift+M` continuam funcionando dentro do Teams.
6. Robustez: matar o processo gravando, reabrir e encontrar a sessão `interrupted` com o texto.
7. Desempenho: as metas da seção 4.
8. Rede: download atrás do proxy corporativo, ou importação pelo navegador.
9. Parte 5: EDL de exemplo com 3 cortes gera o `.srt` final, e a verificação dá desvio mediano ≤ ~0,12 s.

---

## 11. Registro de riscos

| # | Risco | Prob. | Impacto | Mitigação | Fase |
|---|---|:---:|:---:|---|:---:|
| R1 | O crate Rust do sherpa-onnx não expõe timestamps | Baixa | Alto | Confirmar em S1. Plano B: C API via FFI, decidido com você | 0 |
| R2 | App não abre na máquina de quem instala por falta de DLL/runtime C++ | Média | Alto | ADR-003 + Sandbox desde a F1 | 0, 1 |
| R3 | A rede corporativa bloqueia o download (proxy, TLS, GitHub) | Alta | Alto | Certificados do SO, proxy, retomada, **navegador/arquivo local** | 0, 2 |
| R4 | OUTROS não é captado (papel errado, Bluetooth HFP) | Alta | Alto | Dois papéis, acompanhar trocas, medidor + dica, por aplicativo (D7) | 0, 4 |
| R5 | Lacunas do loopback desalinham a linha do tempo | Alta | Médio | Relógio QPC + preenchimento | 0, 4 |
| R6 | Antivírus/EDR/SmartScreen bloqueia ou alerta. **Confirmado no S3:** o Kaspersky segura a captura de áudio (mic e loopback) até o usuário aprovar, e pede de novo a cada exe novo, ou seja, a cada atualização | **Alta** (era Média) | Alto | Falar com o TI, hash publicado, assinatura (D4), nada de ofuscação. Captura não bloqueante com o estado "aguardando permissão" na UI (Fase 4). Pedir ao TI uma exceção por publicador assinado, que sobrevive às atualizações, e não por hash | 4, 6 |
| R7 | Uso sem avisar os participantes (LGPD/compliance) | Média | Alto | Tela de privacidade no 1º uso, áudio desligado por padrão, dados locais, botão para copiar um aviso pronto para o chat ("esta reunião está sendo transcrita localmente") | 2, 5 |
| R8 | Atalho global conflita com o app de reunião | Alta (era) | Médio | Padrões sem conflito, configuráveis, falha de registro detectada | 1 |
| R9 | Perfil Roaming inchado | Alta (era) | Médio | `%LOCALAPPDATA%` | 1 |
| R10 | CPU disputada com o Teams faz o áudio engasgar | Média | Médio | 2 threads ao vivo, MMCSS, meta de CPU medida | 4 |
| R11 | Proteção de conteúdo ignorada por gravador em nível de driver | Baixa | Médio | Aviso na UI | 1 |
| R12 | Eco sem fone | Alta | Médio | Marcar em vez de apagar, recomendar fone, AEC no backlog | 4 |
| R13 | Parakeet com grafia de pt-PT e nomes próprios errados | Alta | Médio | Editor de palavras, candidatos no relatório, Parte 4, dicionário no backlog | 3, 7 |
| R14 | Partes 4/5 atrasam a entrega da v1.0 | Alta (era) | Médio | Reordenação (D1) | — |
| R15 | Meta de RAM irreal com duas WebViews | Alta (era) | Baixo | Janelas sob demanda e metas calibradas em S5 | 0, 1 |
| R16 | Falha nativa do onnxruntime derruba o app na reunião | Baixa | Alto | Gravação por segmento, órfãs, gatilho para processo trabalhador (6.5) | 4, 6 |

---

## 12. Decisões que precisam do seu OK

| # | Decisão | Recomendação |
|---|---|---|
| **D1** | Entregar a v1.0 de reunião (com piloto) **antes** das Partes 4 e 5? O escopo continua o mesmo, só a ordem muda | **Sim** |
| **D2** | Áudio da reunião: padrão **"não salvar"**, com opções "manter 7 dias", "30 dias" e "sempre" | **Não salvar por padrão**: menos disco, menos dado pessoal (voz). Sem áudio, não dá para clicar e ouvir nem usar a Parte 4 em reunião |
| **D3** | Atalhos padrão `Ctrl+Alt+Shift+O` (mini), `+R` (gravar), `+M` (marcar) | **Sim**, configuráveis |
| **D4** | Distribuição do piloto: falar com o TI antes do piloto? Assinar o instalador? | **Falar com o TI antes.** Assinar só se o TI exigir ou se o SmartScreen/EDR bloquear no piloto (tem custo anual). Sem assinatura, publicar o SHA-256. **✅ Aceita em 01/11/2026 (ADR-005)** |
| **D5** | Atualizações na v1.0 | **Sem updater automático.** Botão manual "verificar atualização" (opt-in, única chamada de rede extra) ou só instruções no guia. **✅ Botão manual, em 01/11/2026 (ADR-005)** |
| **D6** | Requisitos mínimos: Windows 10 2004+ x64, 8 GB de RAM; ARM64 fora | **Sim** |
| **D7** | Modo "Por aplicativo" (loopback por processo, Win11) na v1.0 | **Só se o spike S3(e) for simples.** Se não, fica para a v1.1, e o modo Automático com dois papéis já cobre os casos principais |

---

## 13. ADRs previstos (`docs/adr/`)

| ADR | Tema | Fase |
|---|---|:---:|
| 001 | Stack e ASR: Tauri v2, campos reais do sherpa-onnx, fator de tempo real medido | 0 |
| 002 | Captura de áudio: `wasapi` × `cpal`, dois papéis, relógio QPC, loopback por processo | 0 |
| 003 | Empacotamento nativo: estático × DLL + runtime local × delay-load | 0 |
| 004 | Dados locais, retenção e exclusão | 1, 5 |
| 005 | Eco: detecção de fone, correlação, marcação | 4 |
| 006 | Distribuição: instalador, WebView2, assinatura, atualizações | 6 |
| 007 | Forma da Parte 4 (Whisper) | 7 |
| 008 | Nemotron como revisor no lugar do Whisper (**rejeitado**) | 7 |
| 009 | Parakeet ajustado para pt-BR como modelo de fala (0.9.2) | 0.9.2 |

---

## 14. Próximos passos

1. Você responde **D1 a D7** (ou aceita as recomendações).
2. O agente executor começa a **Fase 0** pelo diagnóstico e pede autorização antes de cada download ou instalação (modelo de ~640 MB, Windows Sandbox).
3. Se possível, **separe um momento na rede da empresa** (spike S4) e um **fone Bluetooth** (spike S3b). São os dois cenários com mais chance de surpresa.
4. Ao fim da Fase 0: relatório, os três ADRs e a decisão Go/No-go, antes de qualquer código do app.
