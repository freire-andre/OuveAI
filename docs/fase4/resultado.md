# Fase 4: captura ao vivo e transcrição de reunião

Data: 31/10/2026 · Ryzen 5 5600X (12 threads) · Windows 11 build 26300

## O que entrou

- **`ouveai-audio`**
  - `Capture`: uma thread por fonte, com MMCSS "Pro Audio". O WASAPI em `autoconvert` entrega 16 kHz mono float.
  - Posição pelo QPC (`Timeline`): as lacunas viram silêncio e há tolerância de 200 ms no mic.
  - Troca de dispositivo padrão e perda de dispositivo reabrem sem encerrar a sessão.
  - Watchdog de 2 s ("aguardando o Windows liberar o áudio, antivírus?") e de 60 s ("tentar de novo").
  - Modo Automático do EU: o mic que o app de reunião usa (`IAudioSessionManager2`) ou o padrão Console. Um mic Bluetooth sem uso nunca é aberto, para o fone não cair no perfil de chamada.
  - Aviso de "o app usa outro mic" com o botão "Usar este mic".
  - OUTROS: multimídia + comunicação, somados sem duplicar. Opção "Por aplicativo" no build 20348 ou mais novo.
- **`core::live`**
  - VAD Silero por canal → segmentador → Parakeet (2 threads) → filtro de alucinação → eco → banco a cada segmento.
  - A gravação começa na hora, enquanto o modelo carrega. O modelo fica carregado por 5 min depois de parar.
- **`core::wav`**: um WAV por canal, alinhado ao relógio da sessão. O cabeçalho é regravado a cada segundo, então o arquivo segue válido se o app cair.
- **Banco**
  - Tabela `marks` (estrela, dispositivo, pausa).
  - `add_segment` com o `idx` das palavras contínuo dentro da sessão.
  - Validade do áudio (D2): os áudios vencidos são apagados ao abrir o app.
- **App**
  - Gravar, pausar (solta o mic), retomar, parar e salvar.
  - Mini (420×260): medidores, dispositivos, estados, últimas falas, ⭐ e 👁.
  - Configurações › Áudio: mic, saída, por aplicativo, fone, salvar áudio, "Testar áudio" com som de teste e aviso de permissão do mic.
  - "Testar áudio" também no assistente.
  - Sessões: lista, transcrição EU/OUTROS com marcas, ecos ocultos com um toggle, renomear, abrir o áudio, apagar.
- **CLI**: `ouveai-cli live` (Replay de dois WAVs, `--echo`, `--realtime`) e `ouveai-cli capture` (captura real sem UI).

## Decisões tomadas na fase

| Plano | Feito | Motivo |
|---|---|---|
| `rubato` para 16 kHz | `autoconvert` do WASAPI | Sem dependência nova; o S3 já usava |
| `ringbuf` | `std::sync::mpsc` | Biblioteca padrão; a captura nunca bloqueia |
| Corte forçado em 20 s no ponto de menor energia (L17) | 1ª pausa de 120 ms depois de 10 s; sem pausa, aos 20 s no ponto mais baixo dos **últimos 3 s** | Ver "Atraso" abaixo |

## Replay (90 s, EU + OUTROS, eco simulado: OUTROS no mic com 120 ms e -8 dB)

- Os 3 trechos de vazamento do EU saíram marcados como eco. Toda a fala real do EU ficou visível.
- Os cortes em fala corrida caem entre palavras.

### Atraso (fim da fala → texto na tela), `--realtime`

| Regra de corte | p50 | p95 |
|---|---|---|
| Entre 12 e 20 s (plano original) | 1,89 s | **10,29 s** |
| Pausa curta depois de 10 s / últimos 3 s | **1,52 s** | **2,71 s** |
| Repetição (com medição de CPU/RAM) | 1,54 s | 2,75 s |

Meta: p50 ≤ 3 s e p95 ≤ 6 s. Com a regra antiga, o corte voltava até 8 s no passado, e as palavras desse trecho só apareciam quando o corte acontecia.

### Recursos (Replay em tempo real, máquina livre, carga do modelo incluída)

- **CPU média: 2,4%** da máquina (meta < 20%).
- **RAM no pico: 883 MB** (meta < 1,5 GB).

## Windows Sandbox (instalador NSIS, sem microfone, sem rede)

- A CLI `capture` listou "Áudio Remoto". OUTROS abriu, e o EU sem dispositivo deu erro 0x80070490 sem travar nada.
- Pela UI, por UI Automation:
  - Gravar → mini com "nenhum microfone encontrado" / "Áudio Remoto" e modelo pronto em 2,6 s;
  - ⭐ → Pausar → Retomar → Parar;
  - a sessão aparece com a marca e a pausa, 17 s, e "Abrir áudio (até 07/11)";
  - WAVs de 17,4 s, alinhados ao relógio com a pausa em silêncio;
  - Configurações › Áudio e "Testar áudio" funcionaram.
- **Bug achado e corrigido:** criar a mini dentro de um comando síncrono travava a thread principal do Tauri. Agora o início da gravação roda fora dela, e os comandos que abrem janela são `async`.
- Log (regra 8): só contagens, tempos e nomes de dispositivo.

## Validação do usuário (01/11/2026): ok

Validado pelo usuário na máquina dele, com o instalador da Fase 5:
- chamadas reais no Teams, no Meet e no Zoom: EU e OUTROS separados, texto na mini, ⭐, pausa e invisível no compartilhamento;
- fone com fio conectado e desconectado no meio, e troca de perfil do Bluetooth (entrar e sair da chamada), sem derrubar a sessão.

Ficou sem medir: a CPU média numa chamada real. Entra nas medições finais da Fase 6 (seção 4).
