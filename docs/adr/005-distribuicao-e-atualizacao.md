# ADR-005: Distribuição do piloto e atualização manual

- **Status:** aceito (Fase 6), em 01/11/2026
- **Relacionados:** D4 (distribuição), D5 (atualizações), D6 (requisitos), ADR-003 (empacotamento), R6 (antivírus/EDR)

## Decisões

1. **D4: instalador sem assinatura, com SHA-256 publicado.**
   - O `scripts/release.ps1` roda os testes, gera o instalador e grava `<instalador>.sha256`.
   - Antes do piloto, o TI recebe `docs/piloto/mensagem-ti.md`, que diz o que o app faz, para onde ele se conecta e quais comportamentos o EDR pode sinalizar.
   - A assinatura de código fica para quando o TI exigir ou o SmartScreen/EDR bloquear no piloto (tem custo anual).
2. **D5: atualização manual.** O botão **Verificar atualização** (Configurações › Sobre):
   - faz um `GET` a `api.github.com/repos/freire-andre/OuveAI/releases/latest`, só quando a pessoa clica, com o proxy do app;
   - compara a versão e, se houver uma mais nova, abre a página da versão no navegador;
   - não baixa nem instala nada, e só abre links do próprio repositório.

   Sem updater automático: não há chave de assinatura das atualizações para guardar, nem chamada de rede periódica, e a lista do que sai da máquina (seção 6.7) continua fechada.
3. **Instalador NSIS**, por usuário, sem administrador.
   - **SO mínimo (D6):** o gancho `NSIS_HOOK_PREINSTALL` recusa builds anteriores ao 19041, com uma mensagem clara.
   - **Desinstalador:** a caixa padrão do Tauri apaga `%LOCALAPPDATA%\br.com.ouveai` (reuniões, áudio, modelo, logs e o perfil do WebView2). O texto da caixa virou "Apagar também reuniões, áudios e o modelo baixado". Ela vem desmarcada, e uma desinstalação silenciosa mantém os dados.
   - **Idioma:** `windows/PortugueseBR.nsh` substitui o arquivo de idioma do Tauri: troca os `{{product_name}}`, corrige erros de digitação e o texto da caixa. Ao atualizar o `tauri-cli`, comparar com o arquivo gerado em `target/release/nsis/x64/`, que pode trazer textos novos.
   - **WebView2:** bootstrapper baixado só se faltar (padrão). Embutir fica para quando a rede do piloto bloquear.
   - **A CLI não vai no instalador**, que leva só `ouveai.exe`.
4. **Exportar diagnóstico (.zip)**, em Configurações › Sobre:
   - `relatorio.txt`: versão, Windows, processador, RAM, disco, permissão do microfone, estado do modelo e do FFmpeg, contagens, dispositivos de áudio e configurações (o proxy perde usuário e senha);
   - mais os logs.

   Sem texto, áudio, títulos ou nomes de arquivo (regra 8). O zip é gravado num `.part` e renomeado, então uma falha não deixa um arquivo pela metade.
5. **Versão do piloto: 0.9.0.** A 1.0.0 sai quando o piloto cumprir os critérios de saída.
