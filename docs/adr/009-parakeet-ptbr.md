# ADR-009: Parakeet ajustado para pt-BR como modelo de fala

- **Status:** aceito (0.9.2), em 02/11/2026
- **Relacionados:** ADR-001 (stack de ASR), ADR-006 (comparação com o Whisper), ADR-008 (Nemotron, rejeitado), [S9](../fase0/s9-parakeet-ptbr.md), [catálogo](../fase0/catalogo-modelos.md)

## Contexto

O Parakeet TDT 0.6b v3 é multilíngue e não aceita indicação de idioma. Em trechos com termos e nomes em inglês, ele às vezes troca de idioma no meio da frase. A Fase 3 já tinha visto isso com pedaços curtos, e o S9 viu de novo numa reunião simulada. Parte do treino dele também é português de Portugal (risco R13).

## Decisões

1. **Modelo de fala: o ajuste pt-BR "30s".**
   - É o fine-tune `alexandreacff/parakeet-tdt-v3-ptBR-30s`, treinado com o conjunto TAGARELA, convertido para o sherpa-onnx int8 pelo script oficial (`joaorura/parakeet-tdt-0.6b-v3-ptBR-sherpa-onnx-int8`, commit fixado).
   - Arquitetura, quantização e `tokens.txt` são os mesmos do base, então o código de reconhecimento não muda.
   - No S9, o podcast de 65 min caiu de 13,1% para 11,8% de WER. Os áudios curtos caíram de 50 para 30 erros, e nenhum trecho saiu em inglês.
   - Licença CC-BY-4.0 em toda a cadeia, com crédito na tela Sobre e no README.
2. **Descartados:**
   - **O pt-BR "plus" (exports onnx-asr do mesmo conjunto):** apaga a segunda voz quando há fala sobreposta.
   - **O fp32:** derivou para o inglês e ocupa 2,5 GB.
   - **A normalização de volume:** não teve efeito com o modelo pt-BR.
3. **Pedaços continuam de até 60 s** (corte no silêncio entre 45 e 60 s, Fase 3). Com o modelo pt-BR, 30 s empatou (11,9% contra 11,8% no podcast). O ganho de 30 s no base vinha de evitar a deriva para o inglês, e o pt-BR já não tem isso.
4. **Transição da 0.9.1.**
   - O id novo é `parakeet-tdt-0.6b-v3-ptbr-int8`. O multilíngue (`parakeet-tdt-0.6b-v3-int8`) fica no catálogo como modelo anterior (`legacy`).
   - `models::speech_model()` usa o pt-BR. Sem ele instalado, usa o multilíngue, então quem atualiza não perde a transcrição antes de baixar o novo.
   - A tela Modelos só mostra o multilíngue se ele estiver no disco, com o aviso de que pode ser apagado depois do download do novo.
5. **Cópia no release do próprio projeto.**
   - O pacote fica em `parakeet-tdt-0.6b-v3-ptbr-int8.tar.bz2`, no release [`asr-models`](https://github.com/freire-andre/OuveAI/releases/tag/asr-models) do OuveAI. São os arquivos sem modificação mais um `README.txt` com os créditos, como a CC-BY-4.0 exige.
   - É a fonte principal do download, conferida pelo SHA-256 do pacote e de cada arquivo.
   - O repositório de origem no Hugging Face fica como espelho, fixado por commit.
   - Os destinos de rede continuam os mesmos da 0.9.1 (GitHub, com o Hugging Face como alternativa), e o "Baixar pelo navegador" e a importação do `.tar.bz2` funcionam como antes.
   - O release `asr-models` não é marcado como o mais recente, para não confundir a verificação de atualização.

## Consequências

- O R13 (grafia de Portugal) diminui, mas os candidatos de pt-PT continuam no relatório.
- O download não depende mais de terceiros. Se o repositório de origem no Hugging Face sumir, só se perde o espelho.
- O fine-tuning com áudio do próprio domínio fica para depois do piloto. Ele exige GPU e de 5 a 20 horas ou mais de transcrição revisada.
