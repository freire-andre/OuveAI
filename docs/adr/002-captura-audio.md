# ADR-002 · Captura de áudio: `wasapi`, dois papéis, QPC, mic da reunião, loopback por processo

- **Status:** aceito (Fase 0, spike S3) · 31/10/2026
- **Relatório:** [`docs/fase0/s3-captura.md`](../fase0/s3-captura.md)

## Decisões

1. **Crate `wasapi` (0.26), não `cpal`.** Ele expõe o que o OuveAI precisa e o `cpal` esconde:
   - QPC por pacote (`BufferInfo.timestamp`);
   - padrão por papel (`get_default_device_for_role`);
   - `IMMNotificationClient` (`register_notification_callback`);
   - sessões de áudio por dispositivo;
   - loopback por processo (`new_application_loopback_client`).

   O crate `windows` 0.62 (mesma versão do `wasapi`) cobre MMCSS, property store e ToolHelp.
2. **OUTROS no modo Automático** = loopback do padrão **multimídia** + do padrão de **comunicação**, deduplicados pelo id do endpoint e somados. Cada um reabre sozinho na troca de padrão ou em `AUDCLNT_E_DEVICE_INVALIDATED`.
3. **OUTROS "Por aplicativo"** (Win11 / build 20348+) como opção avançada na v1.0. Funcionou no spike e isolou o Zoom do YouTube.
4. **EU = o mic que o app de reunião está usando**, detectado pelas sessões de captura ativas (`IAudioSessionManager2`).
   - Sem app detectado: o **dispositivo padrão** (papel Console), não o de comunicação.
   - **Nunca abrir sozinho um mic Bluetooth** (Headset/Handset) que nenhum app esteja usando, porque isso força o HFP.
5. **Relógio da sessão:** cada pacote é posicionado pelo QPC, com **tolerância de ~200 ms**.
   - Abaixo da tolerância, a faixa segue contínua (o jitter do mic Bluetooth não vira corte).
   - Acima, preenche com silêncio (lacunas reais do loopback) ou descarta a sobreposição.
6. **Formato:** shared + eventos + `autoconvert` para float mono, sem exclusivo. O reamostrador para 16 kHz (`rubato` ou o próprio `autoconvert`) é decidido na Fase 4 com o golden test.
7. **Abertura nunca bloqueia a UI nem as outras fontes:** o `Initialize` pode esperar dezenas de segundos pelo antivírus. Uma thread por fonte, com o estado "aguardando permissão" exposto à UI.

## Consequências

- `ouveai-audio` fica restrito ao Windows (`cfg(windows)`). O core recebe amostras e posições já no relógio da sessão.
- A UI precisa de:
  - medidor por canal;
  - aviso de OUTROS mudo;
  - aviso de "aguardando permissão";
  - aviso de "o app de reunião usa outro mic".
- Checklist E2E da Fase 4: Teams desktop, Meet com 2º participante e desconexão do dispositivo de saída, que não foram cobertos no S3.
