# Fase 1 · Esqueleto do app + instalador mínimo

> 31/10/2026 · Tauri 2.12.5 · Svelte 5.57 + Vite 8.3 + TS 6.0 · instalador NSIS por usuário (2,7 MB)

## O que existe

- **Workspace:** `ouveai-core` (`paths`, `store`), `ouveai-audio` (spike S3), `ouveai-cli` (`ouveai paths`), `src-tauri` (app). Frontend na raiz (`src/`).
- **Dados:** `%LOCALAPPDATA%\br.com.ouveai\` (`ouveai.db`, `logs\`, e depois `models\`, `bin\`, `recordings\`). Igual ao `app_local_data_dir` do Tauri; `OUVEAI_DATA_DIR` sobrepõe.
- **Banco:** SQLite (WAL, `synchronous=NORMAL`) com migrações por `user_version`. Na Fase 1, só a tabela `settings`.
- **Janelas sob demanda:**
  - nenhuma WebView na bandeja; `main` e `mini` nascem ocultas e aparecem quando a página termina de carregar;
  - a principal é destruída ao fechar (o app fica na bandeja);
  - a mini: 360×220, sem moldura, transparente, sempre no topo, fora da barra de tarefas, **não pega foco** (`WS_EX_NOACTIVATE`, L20) e é arrastável;
  - a posição da mini é salva e validada contra a área útil dos monitores (L19).
- **Stealth:**
  - `WDA_EXCLUDEFROMCAPTURE` em todas as janelas, ligado por padrão;
  - em builds < 19041 começa desligado e a tela avisa;
  - alterna pela bandeja e pelas Configurações.
- **Bandeja:** Abrir · Mostrar/ocultar mini · Gravar/Parar · Stealth · Configurações · Sair. O clique esquerdo abre a principal.
- **Atalhos (D3):** `Ctrl+Alt+Shift+O/R/M`, configuráveis.
  - Uma falha de registro é exibida na tela, que sugere outra combinação.
  - Atalhos duplicados são recusados.
  - Os atalhos ficam suspensos enquanto a tela captura uma combinação nova.
- **Instância única:** a segunda instância foca a principal.
- **Logs:** `logs\ouveai.log`, com rotação de 2 MB × 5. Só eventos, nunca conteúdo (regra 8).
- **Gravar** por enquanto só abre ou fecha a mini e registra marcas. A captura entra na Fase 4.

## Medições

Método do S5: soma do *Working Set – Private* do app e dos processos do WebView2.

| | Esta máquina | Sandbox limpo (Win11 26100) | Meta |
|---|---|---|---|
| Pronto na bandeja (desde a criação do processo) | 113–126 ms | 39 ms | abrir < 1 s ✅ |
| RAM na bandeja | 1,7 MB | 1,7 MB | < 30 MB ✅ |
| RAM com a mini | 74 MB | 74 MB | — |
| RAM com a principal | 80 MB | 77 MB | < 120 MB ✅ |
| RAM depois de fechar as janelas | 3,9 MB | 4,0 MB | volta à bandeja ✅ |
| Abrir a mini (até a página pronta) | 361 ms | 536 ms (1ª) / 185 ms | < 600 ms ✅ |
| Abrir a principal | 154–387 ms | 441 ms | — |

## Máquina limpa (Windows Sandbox, sem rede)

✅ O instalador `/S` rodou sem admin em 2 s.
- O exe ficou em `%LOCALAPPDATA%\OuveAI\`, sem nenhuma DLL ao lado, e o atalho no Iniciar foi criado.
- A sequência mini → gravar → marcar → parar funcionou.
- A segunda instância abriu a principal.
- O stealth foi desligado pelo teclado na tela de Configurações.
- O desinstalador `/S` removeu o exe, e nada foi para o Roaming.
- Prints da tela do Sandbox:
  - stealth ligado → a mini e a principal **não aparecem**, mas o ícone fica na barra de tarefas;
  - stealth desligado → as duas aparecem.

## Correções durante a fase

- A janela principal às vezes abria minimizada (o 1º `ShowWindow` do processo herda o modo de início). Agora a restauração roda na fila da thread da janela, depois do `show()`.
- Os símbolos da mini (↗ virava emoji) foram trocados por ícones SVG.
- O TypeScript 7 é incompatível com o `svelte-check` 4.7, então o projeto fixa o TypeScript 6.0.3.

## Verificação manual (usuário, 31/10/2026) ✅

Teste no **Teams** e no **Google Meet**:
- compartilhando a tela, a mini e a janela principal não aparecem para os outros participantes;
- `Ctrl+Shift+O` do Teams continua funcionando;
- a mini não atrapalha a reunião.

## Observações

- O instalador também cria um atalho na Área de Trabalho (padrão do Tauri). Para um app de bandeja, talvez seja melhor só o Iniciar. Fica para decidir na Fase 6.
- O ícone ainda é provisório.
- `npm audit`: 2 alertas baixos no `deepmerge`, que é dependência de build do plugin do Svelte e não vai para o app.
