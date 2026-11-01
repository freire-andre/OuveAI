# ADR-004: Dados locais, validade do áudio e exclusão

- **Status:** aceito (Fases 1, 4 e 5), em 01/11/2026
- **Relacionados:** D2 (salvar o áudio), L1 (`%LOCALAPPDATA%`), L22 (apagar dados), regra 8 (logs sem conteúdo)

## Decisões

1. **Tudo fica em `%LOCALAPPDATA%\br.com.ouveai`**, nunca no Roaming:
   - `ouveai.db` (SQLite com WAL e `synchronous=NORMAL`);
   - `models\`, `bin\` (FFmpeg), `recordings\<id da sessão>\` e `logs\`.

   Em perfil corporativo, o Roaming sincroniza com o servidor. O modelo (~640 MB) e os WAVs deixariam o logon lento e estourariam a cota.

2. **Áudio da reunião (D2): "não salvar" é o padrão.** As outras opções são 7 dias, 30 dias ou "sempre".
   - Um WAV PCM16 de 16 kHz por canal (`eu.wav`, `outros.wav`), alinhado ao relógio da sessão. A pausa vira silêncio.
   - O cabeçalho é regravado a cada segundo. Se o app cair, o arquivo continua tocável.
   - A pasta é registrada no banco **assim que a gravação começa**, não só ao parar. Uma sessão interrompida continua achando o próprio áudio.

3. **Validade**
   - `audio_expires_at` é gravado ao parar, ou, numa sessão interrompida, na recuperação ao abrir o app. Vale a opção configurada naquele momento.
   - Os áudios vencidos são apagados ao abrir o app e depois de hora em hora (`take_expired_audio`).
   - O banco é limpo primeiro e a pasta é apagada depois: se a exclusão falhar, a sessão não aponta para um áudio que pode já não existir.
   - **Defesa:** só são apagadas pastas dentro de `recordings\`. Um caminho estranho no banco nunca leva a apagar outra coisa.

4. **Sessões órfãs:** as que estavam gravando ou pausadas quando o app caiu viram `interrupted`.
   - O texto fica preservado até o último segmento, porque cada fala é gravada numa transação.
   - A duração vira o fim da última fala.

5. **Busca:** FTS5 *external content* sobre `segments`, com `unicode61 remove_diacritics 2`.
   - Os triggers de INSERT, UPDATE do texto e DELETE mantêm o índice. O DELETE também dispara no `ON DELETE CASCADE` da sessão.
   - A migração reconstrói o índice (`rebuild`) para os bancos que já tinham falas.
   - A consulta é montada pelo app: cada termo vira prefixo e todos precisam aparecer. A pessoa nunca escreve sintaxe FTS, então aspas ou `*` não quebram a busca.
   - Os ecos ficam fora dos resultados.

6. **Exclusão**
   - **"Apagar"** numa sessão remove o texto, as marcas e o áudio dela.
   - **"Excluir tudo"** (Configurações › Dados neste computador) apaga todas as sessões, de reunião e de arquivo, e esvazia `recordings\`.
     - Ficam: as configurações, os modelos e os `.srt/.txt/.json` que o usuário salvou ao lado dos vídeos, porque esses arquivos são dele.
     - É recusado durante uma gravação ou uma importação.
     - Pede confirmação em dois passos.
   - O desinstalador perguntar se apaga os dados fica para a Fase 6.

7. **Logs (regra 8):** só contagens, tempos e nomes de dispositivo. Nunca texto, título de sessão ou nome de arquivo. O "Excluir tudo" registra só a quantidade de pastas apagadas.

## Fora desta versão

- Banco criptografado (DPAPI/SQLCipher) e áudio em FLAC (backlog).
