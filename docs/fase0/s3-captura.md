# Spike S3 · Captura de áudio (EU = mic · OUTROS = loopback)

> 30–31/10/2026 · `crates/ouveai-audio/examples/s3_capture.rs` (wasapi 0.26) · roteiro `spikes/s3/cenarios.ps1`
> Máquina: Win11 26H2 · saída Sharkoon DAC (USB, fone) · mics USB PnP (o que o usuário usa), webcam C920, Sharkoon · Bluetooth EDIFIER W800BT Plus (headphone com mic) · Kaspersky ativo.
> Gravações em `testdata/s3/` (não versionadas). A checagem de conteúdo foi feita transcrevendo cada faixa com o Parakeet (S1).

## Como o spike funciona

- Uma thread por fonte, com MMCSS "Pro Audio", modo shared por eventos, `autoconvert` para float 48 kHz mono.
  - `mic`: padrão de entrada, papel Console.
  - `loop_mm`: loopback do padrão multimídia.
  - `loop_comm`: loopback do padrão de comunicação. Fica ocioso quando é o mesmo dispositivo do `loop_mm`.
  - `proc`: loopback de um processo, com a árvore dele.
- Cada pacote é posicionado pelo **QPC do `GetBuffer`** (`BufferInfo.timestamp`, 100 ns) em relação ao início da sessão. Lacunas viram silêncio; pacote atrasado é cortado.
- `IMMNotificationClient` (troca de padrão) incrementa uma "geração", e a thread reabre no novo dispositivo. Erro de leitura/espera (ex.: `0x88890004 AUDCLNT_E_DEVICE_INVALIDATED`) também reabre, sem encerrar a sessão.
- A cada 3 s o spike registra quais apps captam de cada mic (`IAudioSessionManager2`, sessões ativas).
- Lê o `PKEY_AudioEndpoint_FormFactor` (Headphones, Headset, Speakers…).

## Resultados por cenário

| Cenário | Resultado |
|---|---|
| **(c) silêncio** | ✅ O loopback **para de entregar pacotes** quando nada toca (5,4 s e 8,3 s sem pacotes). Com o QPC, as faixas fecharam com o relógio (`loop_mm` 59,79 s × `mic` 59,56 s). Contando amostras, OUTROS teria perdido ~19 s (confirma L4/R5). Vídeo pausado ≠ nada tocando: com pausa, o Chrome segue mandando pacotes de silêncio. |
| **(a) multimídia ≠ comunicação** | ✅ O spike captou os dois endpoints. O **Zoom tocou no padrão (Sharkoon), não no de comunicação (EDIFIER)**. Captar os dois papéis cobre isso sem precisar saber qual o app usa. |
| **(b) Bluetooth entrando em chamada** | ✅ OUTROS seguiu com sinal o tempo todo. ⚠️ **O próprio spike forçou o HFP:** ao conectar o EDIFIER, o Windows o fez mic padrão, o spike abriu esse mic para o EU e o fone ficou "abafado" até o spike terminar. Enquanto isso, o Zoom usava o USB PnP (`mics_em_uso`). |
| **(d) desconectar/reconectar** | ✅ Ao tirar o mic USB, o EU foi para a C920 em 35 ms. Ao reconectar, voltou ao USB, se recuperou de um `0x88890004` em 20 ms e a sessão seguiu (5 aberturas, sem queda). Ao ligar o EDIFIER, o padrão virou ele e o EU seguiu (de novo forçando o HFP). |
| **(e) por aplicativo (Zoom)** | ✅ A faixa `proc` tem **só o Zoom** (−120 dB enquanto só o YouTube tocava; a transcrição é só o eco do teste do Zoom). O `loop_mm` tem os dois. Trocas de dispositivo no meio não afetaram o `proc`. A API é trivial (`AudioClient::new_application_loopback_client(pid, true)`). |
| **Qual mic a reunião usa** | ✅ As sessões de captura mostram o app e o mic (`Microfone USB PnP: Zoom.exe`) e acompanham a troca de mic do Zoom. |
| **(f) mic BT em chamada × tolerância** | Com tolerância de 10 ms entre o QPC e a posição escrita, o jitter do mic HFP (~4 descontinuidades/s) virou ~200 cortes ou inserções em 90 s, e a transcrição saiu vazia ou em "inglês". **Com 200 ms (f2), a transcrição saiu perfeita**, palavra por palavra, apesar de 190 descontinuidades. |

## Qualidade do conteúdo (transcrição com o Parakeet)

- OUTROS por loopback (vídeo do YouTube): praticamente perfeito.
- EU no USB PnP: correto ("teste, um, dois, três…").
- EU no mic Bluetooth HFP com tolerância de 200 ms: correto, inclusive "Verônica" e "Guilherme".

## Achados que mudam o plano (Fase 4)

1. **EU = o mic que o app de reunião está usando.** Detectar pelas sessões de captura ativas (Teams, Zoom, navegador). Sem app detectado, usar o *dispositivo padrão* (Console), **não** o de comunicação: nesta máquina o de comunicação é a C920, onde ninguém fala.
2. **Nunca abrir sozinho o mic de um fone Bluetooth** (form factor Headset/Handset, sem app de reunião captando dele). Isso força o HFP e piora o som que a pessoa ouve durante a reunião inteira. Nesse caso, perguntar ou esperar o app de reunião abrir o mic.
3. **Posicionamento por QPC com tolerância de ~200 ms:**
   - a faixa segue contínua pela contagem de amostras;
   - o QPC só corrige deriva maior que a tolerância e preenche as lacunas reais (loopback em silêncio, troca de dispositivo).
4. **Abertura não bloqueante:** `IAudioClient::Initialize` levou de 3 a 65 s com o Kaspersky esperando aprovação. Cada fonte tem sua thread; EU nunca espera OUTROS. A UI mostra "aguardando permissão".
5. **Loopback por aplicativo funciona** → pelo D7, entra na v1.0 como opção avançada "Por aplicativo".
6. **Dedupe por id de endpoint** entre os padrões multimídia e comunicação funcionou. A soma dos dois em OUTROS fica para a Fase 4.

## Bugs do próprio spike (corrigidos)

- Tolerância maior expôs um índice fora do intervalo ao cortar um pacote atrasado (panic na thread do mic no f1). Corrigido com `min(samples.len())`.
- O roteiro dos cenários: BOM para o PowerShell 5.1, `-So a,b` como texto único, continuar após interrupção.

## Não testado (não bloqueia o Go)

- **Teams desktop:** não instalado nesta máquina.
- **Meet:** precisa de um segundo participante.
- **Desconectar o dispositivo de saída (Sharkoon) com OUTROS ativo:** no (d) o usuário desconectou o mic USB. A reabertura de render foi exercitada no (e), quando o padrão de saída mudou (2 reaberturas em ~20 ms).

→ Cobrir os três no piloto ou no início da Fase 4 (checklist E2E).
