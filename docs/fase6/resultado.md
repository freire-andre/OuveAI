# Fase 6: distribuição e preparação do piloto

Data: 01/11/2026 · versão **0.9.0** · decisões D4 e D5 em [ADR-005](../adr/005-distribuicao-e-atualizacao.md)

## O que entrou

- **Instalador NSIS 0.9.0** (9,1 MB), por usuário, sem administrador. Leva só `ouveai.exe` e `uninstall.exe`, sem a CLI.
  - **SO mínimo:** recusa builds anteriores ao 19041 (Windows 10 2004), com uma mensagem clara.
  - **Desinstalador:** a caixa "Apagar também reuniões, áudios e o modelo baixado" vem desmarcada.
  - **Idioma:** textos do instalador em pt-BR corrigidos (`src-tauri/windows/PortugueseBR.nsh`).
- **Configurações › Sobre**
  - **Verificar atualização** (D5): consulta o GitHub Releases só quando a pessoa clica e abre a página da versão.
  - **Exportar diagnóstico…** gera um `.zip` com `relatorio.txt` e os logs. O relatório traz versão, Windows, processador, RAM, disco, permissão do microfone, modelos, FFmpeg, contagens, dispositivos e configurações. O proxy perde usuário e senha. Não leva texto, áudio nem títulos.
  - **Licenças e atribuições:** Parakeet CC-BY-4.0, sherpa-onnx, ONNX Runtime, Silero, FFmpeg GPL-3.0, Tauri, Svelte, SQLite e crates.
- **Rede (Configurações):** o texto agora lista exatamente o que usa a internet.
- **`scripts/release.ps1`** (D4): roda os testes e o `svelte-check`, gera o instalador e grava o `.sha256`. Não publica nada.
- **Documentos do piloto**
  - [Guia rápido de 1 página](../guia.md)
  - [Mensagem para o TI](../piloto/mensagem-ti.md): comportamentos que o EDR pode sinalizar e a lista fechada de destinos de rede
  - [Formulário pós-reunião, triagem e critérios de saída](../piloto/formulario.md)

## Verificação

**Testes automáticos:** 46 no core (eram 42). Os novos cobrem a comparação de versões, a leitura da resposta do GitHub (que recusa link de fora do repositório), o proxy sem credenciais, a data UTC e o zip com relatório e logs. `clippy` está limpo e o `svelte-check` não tem erros.

**Windows Sandbox** (instalador 0.9.0, sem rede, 4 execuções):

| Passo | Resultado |
|---|---|
| Instalar `/S` | exit 0. Instalados só `ouveai.exe` e `uninstall.exe` |
| Abrir até a bandeja | 25–41 ms |
| Janela principal pronta | 527–571 ms (meta < 1 s) |
| RAM só na bandeja | **2,3 MB** (meta < 30 MB) |
| RAM com a janela principal | **80,6 MB** em 7 processos (meta < 120 MB) |
| RAM gravando, com o modelo e a mini | 1054 MB (meta < 1,5 GB) |
| Verificar atualização sem rede | "Sem conexão com o GitHub. Confira a internet ou o proxy (Configurações › Rede)." |
| Exportar diagnóstico | Zip com `relatorio.txt` e `logs/ouveai.log`; o proxy aparece como `http://***@proxy.exemplo:8080` |
| Desinstalar sem marcar a caixa (interface e `/S`) | O app sai; o banco e o modelo ficam |
| Reinstalar | As reuniões e o modelo continuam |
| Desinstalar marcando a caixa | O app sai e `%LOCALAPPDATA%\br.com.ouveai` é apagada |

Na primeira medição, a RAM "só na bandeja" deu 103 MB, mas eram os processos WebView2 do próprio Windows (Pesquisa e Widgets). A medição agora soma só o OuveAI e os processos filhos dele.

**CPU gravando:** no Sandbox deu 1,5% de 12 threads. O número só serve de indicação: só o canal OUTROS abriu (o Sandbox não tem microfone) e a CPU é virtual. A CPU de uma chamada real continua a medir na sua máquina (meta < 20%).

**Não testado:** a recusa do instalador em Windows anterior ao build 19041, porque o Sandbox é build 26100. O gancho foi conferido no `installer.nsi` gerado.

## Pendente antes do piloto

1. ~~O repositório no GitHub era privado.~~ Resolvido em 01/11/2026: o repositório virou público e a release v0.9.0 foi publicada.
2. Mandar a mensagem ao TI (D4) e esperar o retorno.
3. Medir a CPU numa chamada real de 10 min.
4. Convidar 3 a 5 pessoas para o piloto.
