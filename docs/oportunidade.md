# Avaliação e Oportunidades de Melhoria — OuveAI

Documento complementar ao [PLANO.md](file:///F:/Projetos/OuveAI/docs/PLANO.md), focado em usabilidade corporativa, distribuição sem atrito para colegas de trabalho e mitigação de riscos técnicos identificados na análise dos documentos em [docs/](file:///F:/Projetos/OuveAI/docs).

---

## 1. Diagnóstico Geral do Plano Existente

O plano arquitetado no [PLANO.md](file:///F:/Projetos/OuveAI/docs/PLANO.md) é tecnicamente sólido:
- A escolha do **Tauri v2 + Rust + Svelte 5** resolve com precisão o problema de consumo de memória (~40 MB vs ~250 MB do Electron) e inicialização sub-segundo.
- O isolamento de processos e uso do **sherpa-onnx** local cumpre a privacidade total exigida em ambientes corporativos.
- As ideias de produto aproveitadas — como `WDA_EXCLUDEFROMCAPTURE`, pipeline dual EU/OUTROS e SQLite com FTS5 — traz maturidade.

Contudo, foram identificadas **5 oportunidades críticas** que passaram batidas ou precisam de ajuste para garantir que os colegas de trabalho consigam instalar e usar o app sem suporte técnico.

---

## 2. Oportunidades Identificadas

### 🎯 Oportunidade 1: Distribuição e Onboarding "Zero-Setup" para Colegas

**Problema:**  
O plano foca no setup de desenvolvimento (Rust, Cargo, Node LTS, MSVC), mas colegas de trabalho receberão o binário final. Em máquinas corporativas padrão:
1. **Falta do runtime VC++:** Binários nativos do ONNX Runtime e Sherpa no Windows dependem de `msvcp140.dll` e `vcruntime140.dll`. Sem eles, o app fecha silenciosamente com o erro `0xC0000135`.
2. **Bloqueio de rede / Proxy Corporativo (Zscaler, Netskope, Fortinet):** O download do modelo Parakeet (~640 MB) e do FFmpeg diretamente do GitHub Releases falha com erro de TLS se a biblioteca HTTP usar certificados embutidos (`rustls`) em vez do repositório de certificados do Windows.
3. **Download interrompido em conexões lentas:** 640 MB sem retomada (*resume*) pode falhar repetidamente.

**Melhorias propostas:**
- **Instalador NSIS por usuário (`%LOCALAPPDATA%`):** Não exige privilégios de administrador da empresa.
- **Embarcar dependências de C++:** Incluir as DLLs de redistribuição da Microsoft junto ao executável principal ou verificar sua presença no instalador.
- **Suporte a TLS Nativo do Windows:** Configurar o crate HTTP (`reqwest`) com a feature `native-tls` (Schannel), herdando automaticamente as políticas de proxy corporativo e certificados da empresa.
- **Assistente de Primeiro Boot (Wizard Visual):** 
  - Barra de progresso com porcentagem e velocidade.
  - Suporte a download resumível com retentativas automáticas (`Range` headers).
  - Opção alternativa: **"Carregar modelo de arquivo local / pasta de rede"** (permite que você passe o modelo via pendrive ou drive compartilhado caso o proxy da empresa bloqueie o GitHub).

---

### 🎙️ Oportunidade 2: Captura WASAPI Multiplataforma (Teams, Zoom e Google Meet)

**Problema:**  
Cada ferramenta de reunião roteia o som de forma diferente no Windows:
1. **Google Meet (Navegador Chrome/Edge):** O som sai no dispositivo de áudio multimídia padrão do sistema (`eConsole`), como alto-falantes ou saída geral do fone.
2. **Teams e Zoom (Apps Desktop):** Frequentemente utilizam o dispositivo padrão de comunicação (`eCommunications`) ou permitem que o usuário selecione um dispositivo de áudio interno exclusivo nas preferências do app.
3. **Headsets Bluetooth (HFP vs A2DP):** Em reuniões ativas no Zoom/Teams, o Windows muda o perfil do fone Bluetooth para bidirecional (HFP), disparando `AUDCLNT_E_DEVICE_INVALIDATED` e alterando a taxa de amostragem no meio da gravação.

Se o sistema capturar apenas um endpoint fixo (`eConsole` ou apenas `eCommunications`), ele falha ao alternar entre Meet (no navegador) e Teams/Zoom (no desktop).

**Melhorias propostas:**
- **Seletor de Dispositivo de Saída na UI:** Permitir escolher:
  - *Automático (Recomendado):* Detecta se a saída ativa é `eCommunications` (Teams/Zoom) ou `eConsole` (Google Meet no navegador).
  - *Dispositivo Específico:* Lista todos os dispositivos de saída do Windows para seleção manual direta.
- **Recuperação transparente de desconexão (`AUDCLNT_E_DEVICE_INVALIDATED`):** Reconectar automaticamente o stream de loopback caso o headset mude de perfil ou seja desconectado durante a chamada do Zoom/Teams.

---

### 👁️ Oportunidade 3: Refinamento do Modo Stealth no Teams

**Problema:**  
A proteção de conteúdo via `WDA_EXCLUDEFROMCAPTURE` oculta a janela em compartilhamentos de tela, mas possui cenários de borda no dia a dia:
1. **Versões legadas do Windows 10 (< 2004 / build 19041):** Em vez de ficar invisível, o Windows desenha uma caixa preta opaca na transmissão, chamando atenção indesejada.
2. **Perda da janela sem barra de tarefas:** Como a mini-janela tem `skip_taskbar: true`, se o usuário clicar fora dela ou se ela for coberta por uma apresentação em tela cheia do PowerPoint, o colega não sabe como recuperá-la.

**Melhorias propostas:**
- **Checagem de build do Windows na inicialização:** Se a versão for inferior à 19041, desativar `content_protected` por padrão e exibir aviso explicativo ao usuário.
- **Atalho global de resgate:** Definir atalho padronizado (`Alt+Shift+O` ou `Ctrl+Shift+Space`) que traz a mini-janela imediatamente para o topo e centro da tela.
- **Ícone na Bandeja do Sistema (Tray):** Garantir menu de contexto rápido no ícone da bandeja com opções diretas: "Exibir Mini Janela", "Pausar/Gravar" e "Configurações".

---

### 📋 Oportunidade 4: Rebalanceamento de Foco (Reuniões Corporativas vs Edição de Vídeo)

**Problema:**  
O [PLANO.md](file:///F:/Projetos/OuveAI/docs/PLANO.md) incorporou integralmente as Partes 4 e 5 do [prompt.txt](file:///F:/Projetos/OuveAI/docs/inicio/prompt.txt) (comparação com Whisper large e alinhamento de corte de vídeo editado EDL).  
Embora válidas tecnicamente, essas partes atendem a editores de vídeo no DaVinci/Premiere, enquanto o objetivo principal do usuário e de seus colegas é **transcrever reuniões de trabalho no Teams/Meet**.

**Melhorias propostas:**
- **Priorizar utilidades corporativas antes do EDL:**
  1. **Botão "Copiar Ata Formatada":** Copia a transcrição estruturada em Markdown direto para a área de transferência (`[10:15] EU: ...`, `[10:16] OUTROS: ...`), pronta para colar no Slack, Teams ou e-mail.
  2. **Marcador rápido durante a reunião:** Adicionar um botão de atalho na mini-janela (ex.: tecla de espaço ou botão ⭐) para marcar "Momento Importante / Ação", permitindo localizar pontos cruciais no histórico sem reler toda a reunião.
  3. **Política de economia de disco:** Reuniões gravadas geram arquivos WAV pesados (~230 MB/hora). Incluir nas opções: "Armazenar apenas o texto transcrito" ou "Manter áudio bruto por 7 dias".
- Manter a Fase 6 (Whisper) e Fase 7 (EDL) como módulos secundários ou opcionais, sem bloquear a entrega do aplicativo de reunião aos colegas.

---

### 🔊 Oportunidade 5: Heurística de Eco Realista (EU vs OUTROS)

**Problema:**  
O plano prevê descartar falas do microfone que tenham texto parecido (`similar >= 0.8`) com o loopback.  
Na prática:
1. Em reuniões com fone de ouvido, **não há vazamento acústico**. Aplicar essa regra gera falsos positivos (ex.: quando você repete uma frase do colega para concordar: *"Com certeza, vamos seguir assim"*).
2. O microfone capta a voz do alto-falante abafada e com atraso; a taxa de erro do ASR nesse áudio secundário é alta, raramente atingindo 80% de similaridade textual.

**Melhorias propostas:**
- **Chave de detecção de fone:** Adicionar configuração simples na UI: "Estou usando fone de ouvido" (ativada por padrão). Quando ativada, desliga completamente a filtragem de eco textual, preservando 100% da fala do usuário.
- **Porta de volume RMS dinâmica:** Se o usuário estiver sem fone (alto-falantes), utilizar apenas o corte de sensibilidade do VAD durante a reprodução de áudio remoto, sem comparar strings a posteriori.

---

## 3. Matriz de Recomendações e Impacto

| Oportunidade | Impacto | Esforço | Fase Sugerida |
|---|:---:|:---:|:---:|
| **Zero-Setup (Schannel TLS + Wizard de download)** | 🔴 Alto (decisivo para colegas) | Baixo | Fase 2 |
| **Captura WASAPI Multi-app (Teams, Zoom, Meet)** | 🔴 Alto (essencial para reuniões) | Médio | Fase 0 e 4 |
| **Exportação rápida para Clipboard / Markdown** | 🟡 Médio (produtividade imediata) | Muito Baixo | Fase 5 |
| **Atalho de resgate e compatibilidade de Windows** | 🟡 Médio (usabilidade do stealth) | Baixo | Fase 1 |
| **Chave de fone de ouvido (supressão de falso eco)** | 🟡 Médio (fidelidade da transcrição) | Muito Baixo | Fase 4 |

---

## 4. Próxima Ação Imediata

Incorporar os requisitos de **distribuição corporativa** e **WASAPI Multi-app (Teams, Zoom, Meet)** no início da **Fase 0 (Diagnóstico e Spike)** do projeto.
