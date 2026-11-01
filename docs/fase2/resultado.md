# Fase 2 · Modelos, FFmpeg e primeiro uso

> 31/10/2026 · instalador NSIS 4,1 MB · `ureq` 3.5 + `platform-verifier` · `bzip2` 0.6 / `tar` 0.4 / `zip` 9.2 (só `deflate`), todos em Rust puro

## O que existe

**Core (`ouveai-core`)**
- `hash`: SHA-256 pelo BCrypt do Windows (sem `sha2`).
- `sys`: build do Windows, RAM, núcleos, AVX2, espaço livre (sobe até uma pasta que exista).
- `net`: download com `.part`, retomada por `Range` + `If-Range` (ETag guardado ao lado), confere o `Content-Range` e tenta até 5 vezes com espera de 4/8/16/30/30 s. Erros 4xx (exceto 408/429) não tentam de novo. **Cada nova tentativa é avisada à UI** (número da tentativa, espera e se foi falta de conexão ou status do servidor). O log guarda só o host, porque a URL assinada do GitHub funciona como credencial.
- `models`:
  - catálogo com o Parakeet TDT 0.6B v3 int8 (pacote `.tar.bz2` do GitHub + espelho do Hugging Face com os arquivos soltos, commit fixo) e o Silero VAD (**dentro do exe**, gravado na pasta de modelos na primeira execução);
  - estados ausente · baixando · pausado · conferindo · extraindo · pronto · com problema;
  - instalação em `models\.tmp-<id>`, conferência de tamanho + SHA-256 de cada arquivo e troca atômica da pasta (a versão anterior só sai quando a nova está conferida);
  - importação de pacote ou de pasta (pendrive, rede), com a mesma conferência; do pacote só saem os nomes do catálogo;
  - checagem de espaço antes de começar (+200 MB de folga).
- `media`: acha o FFmpeg (primeiro o do PATH, depois `bin\ffmpeg.exe`) e confere se é mesmo um FFmpeg rodando `-version`. Roda **sem janela de console** (`CREATE_NO_WINDOW`). Baixa a build fixa da gyan.dev pelo mesmo downloader e extrai só o `ffmpeg.exe` e a licença (`bin\FFMPEG-LICENSE.txt`).

**App**
- **Tela Modelos:** um cartão por modelo com estado, barra de progresso, velocidade, tempo restante, Pausar/Continuar, Baixar do espelho e Apagar.
  - **Plano B sempre visível** enquanto o modelo não está pronto: "Baixar pelo navegador" (abre a URL do pacote e explica o passo seguinte), "Importar arquivo…" e "Importar pasta…".
  - Também aparece durante as novas tentativas. Importar nesse momento cancela o download e importa em seguida.
- **Cartão FFmpeg** ("Ferramentas"): mostra o encontrado (versão e origem) ou oferece o download, **sempre com confirmação** dizendo o que é, de onde vem, o tamanho e para onde vai. Na Fase 3, o primeiro "Importar arquivo" de áudio usa o mesmo cartão.
- **Configurações › Rede:** proxy manual (`http://servidor:porta`). Vazio = proxy do sistema.
- **Assistente de primeiro uso** (só na primeira execução): Boas-vindas → Privacidade (com o lembrete da LGPD) → Seu computador (Windows, memória, processador/AVX2, espaço, com aviso e explicação) → Modelo (o mesmo cartão, com "Continuar sem esperar") → Atalhos e bandeja.
- **CLI `ouveai-cli`** (renomeada para não colidir com o `ouveai.exe` do app): `models [download|import|verify]`, `ffmpeg [download]`, `config [set]` e `paths`.

## Testes

| Teste | Resultado |
|---|---|
| Testes unitários (`ouveai-core`) | 9 ✅ (SHA-256, pacote importado só com os arquivos do catálogo, adulterado recusado mantendo o anterior, pasta incompleta, Silero = catálogo, FFmpeg extraído do zip, `probe` recusa o que não é FFmpeg, `paths`) |
| Download do Parakeet interrompido e retomado (`.part` de 480 MB) | instalado, hashes ok ✅ |
| `.part` adulterado | recusado pelo SHA-256 e apagado, modelo anterior mantido ✅ |
| Espelho HF interrompido em 158 MB | retomou até 462 MB com os bytes iguais ✅ |
| Importar pasta / pacote `.tar.bz2` | 1,8 s / 37 s ✅ |
| FFmpeg: download, parada em 78 MB, retomada | instalado e executando ✅ |
| FFmpeg: `.part` adulterado | recusado pelo SHA-256 ✅ |
| Proxy apontando para uma porta morta | "sem conexão; tentativa 1/5 em 4 s…" ✅ |

## Máquina limpa (Windows Sandbox)

**Sem rede:**
- ✅ O instalador `/S` rodou sem admin.
- ✅ O assistente abre sozinho no 1º uso (janela pronta em 583 ms) e o Silero é gravado na inicialização.
- ✅ A checagem da máquina mostra o aviso de memória (o Sandbox tem 4 GB).
- ✅ "Baixar" sem rede mostra "Sem conexão com o servidor · tentativa 2 de 5 · tentando de novo em 8 s", a dica e o plano B. Antes ficava ~90 s em "Conectando…".
- ✅ O import pela CLI levou 3–4 s, e os dois modelos ficaram prontos.

**Com rede:**
- ✅ Tela Modelos → "Baixar FFmpeg…" → confirmação → baixado, conferido e extraído em 4 s.
- ✅ O cartão passa a "Pronto · baixado pelo OuveAI".
- ✅ Em `bin\` ficaram só `ffmpeg.exe` (100,5 MB) e a licença.

Os prints só saem com o stealth **desligado**: com ele ligado, a captura de tela do Sandbox pega só a área de trabalho. Por isso o script de teste desliga o stealth (`ouveai-cli config set stealth false`) antes de abrir o app.

## Correções durante a fase

- **Tentativas:** a UI ficava em "Conectando…" durante todas as novas tentativas. Agora mostra a tentativa, a espera e o motivo, e mantém o plano B à mão.
- **Importar durante um download:** dava "já está em andamento". Agora cancela o download e importa.
- **Foco do teclado:** ao clicar em Baixar, o foco ia para o botão Pausar. Na confirmação do FFmpeg, o foco já cai em "Baixar".
- **FFmpeg:** o `.etag` e a pasta `_downloads` vazia sobravam depois de concluir ou de recusar um pacote.
- **Corrida entre testes por variáveis de ambiente:** resolvida com um `ENV_LOCK` compartilhado.

## Critérios do plano

- ✅ Um download interrompido retoma sem corromper e um arquivo adulterado é recusado.
- ✅ A importação de arquivo funciona sem internet.
- ✅ A UI mostra o estado de cada modelo.
- ✅ Rede da empresa: o S4 rodou no notebook corporativo (sem proxy, sem inspeção de HTTPS, VPN só para o SAP, 206 nos dois hosts) e o plano B está pronto. O app em si ainda não foi instalado lá; se você quiser, é o mesmo instalador.

## Fica para depois

- **Fase 3:** pedir o FFmpeg no primeiro "Importar arquivo" de áudio, usando o cartão desta fase.
- **Fase 6:** ícone definitivo (a partir das suas imagens em `image/`), atalho na Área de Trabalho, Defender/SmartScreen com exe sem assinatura e a tela Sobre com as licenças (Parakeet CC-BY-4.0, Silero MIT, FFmpeg GPL).
- O "tentando de novo em N s" é fixo, sem contagem regressiva. Dá para animar depois, se incomodar.
