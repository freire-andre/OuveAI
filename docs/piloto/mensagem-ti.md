# Mensagem para o TI (antes do piloto)

> Texto pronto para enviar ao TI/segurança. Preencha o que está entre ‹ ›. Mande o **SHA-256** gerado pelo `scripts/release.ps1` junto com o instalador.

---

**Assunto:** Piloto do OuveAI (transcrição local de reuniões): pedido de avaliação

Olá,

Quero fazer um piloto de ~2 semanas com ‹3–5› colegas usando o **OuveAI**, um app que desenvolvi para transcrever reuniões **localmente**, sem enviar áudio nem texto para nenhum serviço. Antes de distribuir, gostaria da avaliação de vocês, porque alguns comportamentos dele costumam chamar a atenção de antivírus/EDR.

**O que é**
- App desktop para Windows 10 2004+ (x64), feito com Tauri (Rust + WebView2).
- Reconhecimento de fala offline com o modelo Parakeet (NVIDIA, CC-BY-4.0) na versão ajustada para português do Brasil, via sherpa-onnx/ONNX Runtime, rodando na CPU.
- Código-fonte (público): https://github.com/freire-andre/OuveAI

**Instalação**
- Instalador NSIS **por usuário**, sem administrador, em `%LOCALAPPDATA%\OuveAI`.
- Dados em `%LOCALAPPDATA%\br.com.ouveai`: banco SQLite, modelo (~640 MB), áudio opcional e logs. Nada vai para o perfil Roaming.
- O instalador **não é assinado** digitalmente. Arquivo e hash desta versão:
  - `OuveAI_0.9.5_x64-setup.exe` (9,3 MB): https://github.com/freire-andre/OuveAI/releases/tag/v0.9.5
  - SHA-256: `2777ce31d1d8ea9f273b1141a120f8a89eee72b0310110f1aa309a18a7b74eee`

**Comportamentos que o EDR pode sinalizar (todos esperados)**

| Comportamento | Por quê |
|---|---|
| Captura de áudio do microfone e do som do sistema (WASAPI, loopback) | É a função do app: EU = microfone, OUTROS = som da reunião. Só durante a gravação, iniciada pelo usuário |
| Atalhos globais de teclado (`Ctrl+Alt+Shift+R/M/O`) | Gravar, marcar e mostrar a janela com o app em segundo plano |
| Janela excluída da captura de tela (`SetWindowDisplayAffinity`, `WDA_EXCLUDEFROMCAPTURE`) | A mini não aparece quando o usuário compartilha a tela na reunião. Pode ser desligado no app |
| Lista de processos com sessão de áudio ativa | Descobrir qual app de reunião está tocando, para escolher o dispositivo certo |
| Ícone na bandeja do sistema | Acesso rápido |

Ele **não** se inicia com o Windows, **não** instala serviço nem driver e **não** pede elevação.

**Acesso à rede (lista fechada)**

| Destino | Quando |
|---|---|
| `github.com`, `objects.githubusercontent.com`, `release-assets.githubusercontent.com` | Download do modelo (uma vez, confirmado pelo usuário) e, se o usuário quiser, do Whisper e do FFmpeg. Desde a 0.9.4, tudo vem dos releases do próprio projeto (`github.com/freire-andre/OuveAI`); o release de origem fica como reserva |
| `huggingface.co` e a CDN dele (`*.cdn.hf.co`) | Alternativa para o download dos modelos |
| `api.github.com` | "Verificar atualização", quando o usuário clica. Se ele ligar o aviso de versão nova (desligado por padrão), também ao abrir o app, **no máximo uma vez por dia** |
| `go.microsoft.com` / Microsoft Edge Update | Instalar o WebView2, só se a máquina não tiver |

Fora isso, não há telemetria nem chamada de rede. Sem internet, o app funciona normalmente depois que o modelo é instalado; o modelo também pode ser importado de um arquivo local.

**O que eu peço**
1. Liberação (ou orientação) para o piloto em ‹N› máquinas.
2. Se o EDR/antivírus exigir, uma **exceção para o caminho** `%LOCALAPPDATA%\OuveAI\ouveai.exe` ou para o hash acima. Observação: no Kaspersky, a captura de áudio pede aprovação a cada exe novo, ou seja, a cada atualização.
3. Se vocês exigirem assinatura de código, me avisem, e eu providencio um certificado antes de distribuir.

Fico à disposição para mostrar o app funcionando.

Obrigado,
‹seu nome›
