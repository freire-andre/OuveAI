# Mensagem para o TI (antes do piloto)

> Texto pronto para enviar ao TI/segurança. Preencha o que está entre ‹ ›. Mande o **SHA-256** gerado pelo `scripts/release.ps1` junto com o instalador.

---

**Assunto:** Piloto do OuveAI (transcrição local de reuniões): pedido de avaliação

Olá,

Quero fazer um piloto de ~2 semanas com ‹3–5› colegas usando o **OuveAI**, um app que desenvolvi para transcrever reuniões **localmente**, sem enviar áudio nem texto para nenhum serviço. Antes de distribuir, gostaria da avaliação de vocês, porque alguns comportamentos dele costumam chamar a atenção de antivírus/EDR.

**O que é**
- App desktop para Windows 10 2004+ (x64), feito com Tauri (Rust + WebView2).
- Reconhecimento de fala offline com o modelo Parakeet (NVIDIA, CC-BY-4.0) via sherpa-onnx/ONNX Runtime, rodando na CPU.
- Código-fonte: ‹https://github.com/freire-andre/OuveAI›.

**Instalação**
- Instalador NSIS **por usuário**, sem administrador, em `%LOCALAPPDATA%\OuveAI`.
- Dados em `%LOCALAPPDATA%\br.com.ouveai`: banco SQLite, modelo (~640 MB), áudio opcional e logs. Nada vai para o perfil Roaming.
- O instalador **não é assinado** digitalmente. Arquivo e hash desta versão:
  - `OuveAI_‹versão›_x64-setup.exe` (‹tamanho› MB)
  - SHA-256: `‹hash›`

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
| `github.com`, `objects.githubusercontent.com`, `release-assets.githubusercontent.com` | Download do modelo (uma vez, confirmado pelo usuário) e do FFmpeg (opcional, só para importar arquivos de vídeo) |
| `huggingface.co` e CDN | Alternativa para o download do modelo |
| `api.github.com` | "Verificar atualização", **só quando o usuário clica** |
| `go.microsoft.com` / Microsoft Edge Update | Instalar o WebView2, só se a máquina não tiver |

Fora isso, não há telemetria nem chamada de rede. Sem internet, o app funciona normalmente depois que o modelo é instalado; o modelo também pode ser importado de um arquivo local.

**O que eu peço**
1. Liberação (ou orientação) para o piloto em ‹N› máquinas.
2. Se o EDR/antivírus exigir, uma **exceção para o caminho** `%LOCALAPPDATA%\OuveAI\ouveai.exe` ou para o hash acima. Observação: no Kaspersky, a captura de áudio pede aprovação a cada exe novo, ou seja, a cada atualização.
3. Se vocês exigirem assinatura de código, me avisem, e eu providencio um certificado antes de distribuir.

Fico à disposição para mostrar o app funcionando.

Obrigado,
‹seu nome›
