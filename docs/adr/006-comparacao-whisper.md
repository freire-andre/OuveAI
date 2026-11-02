# ADR-006: Forma da Parte 4 (comparação com o Whisper)

- **Status:** aceito (Fase 7), em 01/11/2026
- **Relacionados:** Parte 4 do plano, R13 (grafia de pt-PT e nomes próprios), D2 (áudio da reunião), ADR-001 (stack e ASR)

## Decisões

1. **Modelo: Whisper large-v3-turbo int8**, do catálogo do sherpa-onnx, rodando pelo próprio sherpa-onnx, sem Python.
   - **Por que não o large-v3:** são ~3 GB contra ~1 GB, e ele é bem mais lento na CPU. A comparação já custa cerca de metade da duração do áudio com o turbo.
   - **Opcional:** não entra no assistente de primeiro uso. Ele aparece em Modelos com a nota "Opcional", e o painel de comparação oferece o download quando falta.
   - **Fontes:** pacote do GitHub (k2-fsa) com o espelho do Hugging Face fixado por commit, os dois com SHA-256 conferido, como o Parakeet.
2. **Alinhamento: crate `similar`** (Myers), sobre palavras normalizadas: minúsculas, sem acento e sem pontuação.
   - **Formas coloquiais contam como iguais:** tá/está, tava/estava, tô/estou, pra/para, cê/você.
   - **Diferença só de muletas não é mostrada:** né, pô, cara, ali, tal, então, assim, tipo e parecidas. O Whisper costuma omiti-las, e o Parakeet, que transcreve a fala literal, já está certo.
   - **Trechos próximos se juntam:** diferenças separadas por uma palavra igual viram um trecho só.
3. **Janelas de 20 s, cortadas em silêncio medido.** O limite do Whisper é 30 s, mas com 30 s ele entrava em laço (a mesma frase repetida dezenas de vezes) e engolia frases. Medido num recorte de 3 min:

   | Janela | Laços | Palavras engolidas | Velocidade |
   |---|---|---|---|
   | 30 s | 3 | 56 | 2,0× tempo real |
   | **20 s** | **0** | **15** | 1,9× |
   | 15 s | 0 | 52 | 2,0× |

4. **Janela em que o Whisper falha é descartada, não oferecida.** O alinhamento é feito janela a janela, e a janela sai da lista quando:
   - há um laço: 1 a 8 palavras repetidas 3 vezes seguidas, ou 4 a 12 palavras repetidas 2 vezes;
   - o Whisper tem bem mais palavras que o Parakeet (mais de 1,5× + 5, sinal de invenção);
   - o Whisper tem menos da metade (com 8 palavras ou mais no Parakeet), sinal de que engoliu fala.

   A tela diz em quais trechos isso aconteceu.
5. **Tempos sempre do Parakeet, e só medidos.** O Whisper não mede o tempo de cada palavra, e o prompt proíbe interpolar. Por isso:
   - Com o mesmo número de palavras, cada palavra do Whisper fica com o tempo da que substitui.
   - Com número diferente, o trecho vira uma unidade só ("começar logo"), com o início da primeira e o fim da última palavra substituída. A legenda nunca quebra uma unidade.
   - Escolher "(apagar)" remove as palavras.

   Uma primeira versão distribuía o texto proporcionalmente às letras, mas foi trocada por inventar fronteiras.
6. **Reunião: só com o áudio salvo (D2), segmento a segmento.**
   - Cada segmento é comparado com o trecho do `eu.wav` ou do `outros.wav` correspondente, com 0,25 s de folga de cada lado.
   - Ecos ficam de fora, e segmentos de mais de 30 s são pulados.
   - Os segmentos de EU e OUTROS se sobrepõem no tempo, por isso a reunião não usa janelas.
7. **Recursos:** uma comparação por vez.
   - Não começa com gravação em andamento nem com a fila de arquivos rodando. O Whisper sozinho ocupa cerca de 1,7 GB, e com o Parakeet carregado o app chegou a 2,9 GB.
   - Se uma gravação começar no meio, a comparação para (a reunião tem prioridade, L18). A fila de arquivos espera a comparação terminar.
   - O Whisper é carregado no início da comparação e liberado no fim.
   - **Cancelar** vale entre um trecho e outro, porque o Whisper não interrompe um trecho no meio (até ~10 s). A tela mostra "Cancelando…" na hora e some quando termina, sem mensagem de erro.
8. **Aplicar** troca as palavras numa transação (`Store::replace_words`).
   - Os segmentos tocados ganham a marca de editado, e a busca se atualiza pelos triggers do FTS.
   - Antes de trocar, o app confere se o texto ainda é o da comparação. Se a pessoa corrigiu algo no meio, pede para comparar de novo.
   - Em arquivo, o `.json`, o `.txt` e o `.srt` ao lado do original são regenerados, se já existiam.

## Consequências

- **Disco:** +989 MB quando o Whisper é baixado.
- **RAM:** cerca de +1,7 GB durante a comparação, devolvidos ao terminar. Em máquinas de 8 GB, convém fechar outros programas pesados antes.
- **Tempo:** a comparação leva cerca de metade da duração do áudio, contra 1/13 do Parakeet.
- **O que ainda escapa:** os critérios do item 4 pegam os laços e as invenções grandes. Uma invenção curta, de poucas palavras, ainda pode aparecer como trecho divergente, e quem decide é a pessoa.
