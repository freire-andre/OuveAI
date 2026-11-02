# ADR-010: Componentes baixados do próprio repositório

- **Status:** aceito (0.9.4), em 02/11/2026
- **Relacionados:** ADR-005 (distribuição), ADR-006 (comparação com o Whisper), ADR-009 (Parakeet pt-BR), [catálogo](../fase0/catalogo-modelos.md)

## Contexto

O app baixa sob demanda o modelo de fala, o Whisper (opcional) e o FFmpeg (opcional). Até a 0.9.3, só o Parakeet pt-BR vinha do release do próprio projeto. O Whisper vinha do release `asr-models` do k2-fsa/sherpa-onnx, com o Hugging Face como espelho, e o FFmpeg vinha do release `GyanD/codexffmpeg` 9.0.2, sem segunda fonte.

O SHA-256 fixado no código (L14) já impede que uma troca de arquivo na origem passe despercebida: o download é recusado. O risco que sobrava era a origem **sumir ou mudar de endereço**. Nesse caso, o componente não poderia mais ser instalado. Com o repositório público, os releases do GitHub aceitam arquivos de até 2 GB sem custo.

## Decisões

1. **Fonte própria primeiro, origem como reserva.** Cada componente baixado tem uma cópia **sem modificação** (mesmos bytes, mesmo SHA-256) num release do OuveAI. A origem fica como segunda fonte. Uma conta só não vira um novo ponto único de falha.
2. **Whisper:** `sherpa-onnx-whisper-turbo.tar.bz2` no release [`asr-models`](https://github.com/freire-andre/OuveAI/releases/tag/asr-models), com o crédito e a licença MIT nas notas. O espelho continua o Hugging Face fixado por commit (`csukuangfj/sherpa-onnx-whisper-turbo@2ca6ff69`), escolhido pelo usuário em "Usar o espelho", como nos outros modelos.
3. **FFmpeg:** o `.zip` da gyan.dev no release [`ffmpeg-9.0.2`](https://github.com/freire-andre/OuveAI/releases/tag/ffmpeg-9.0.2), que fica fora do "latest" (o aviso de versão nova usa `releases/latest`).
   - **GPL-3.0:** redistribuir o binário obriga a oferecer o código-fonte correspondente. Por isso, o mesmo release traz o `ffmpeg-9.0.2.tar.xz` do ffmpeg.org e uma nota com a licença, o commit declarado pela build (`946fcce07b`) e onde está a configuração e as bibliotecas externas (o `README.txt` do pacote e a página de builds da gyan.dev).
   - O FFmpeg roda como programa à parte e não é ligado ao OuveAI.
   - **Reserva automática:** `media::download` tenta o release do OuveAI e, se ele falhar (fora do ar, erro de rede depois das novas tentativas ou SHA-256 diferente), passa para o release da gyan.dev. Pausar não troca de fonte. O `.part` pode ser retomado na outra fonte, porque o arquivo é o mesmo, e se o servidor recusar o Range o download recomeça do zero.
4. **Fica como está:**
   - **Parakeet multilíngue** (`parakeet-tdt-0.6b-v3-int8`, legado). Ele só existe para a transição da 0.9.1 e não é oferecido para instalação nova.
   - **Silero VAD.** Vai embutido no exe.
   - **Biblioteca do sherpa-onnx usada na compilação.** Ela é baixada pelo crate `sherpa-onnx` só no build, e `SHERPA_ONNX_ARCHIVE_DIR` permite usar uma cópia local. Não afeta quem instala o app.
5. **Atualizar um componente** é decisão explícita: subir o novo arquivo (um release novo para outra versão do FFmpeg), conferir o hash e trocar URL e SHA-256 no código. Nada muda sozinho.

## Consequências

- Todos os downloads da 0.9.4 saem do `github.com/freire-andre/OuveAI`. O Hugging Face e o GyanD ficam só como reserva. Os destinos de rede da mensagem ao TI não mudam.
- As versões 0.9.0 a 0.9.3 continuam com as URLs antigas, que seguem no ar.
- Teste em 02/11/2026, com a CLI e uma pasta de dados vazia: o Whisper pela fonte principal e pelo espelho e o FFmpeg pela fonte principal instalaram com o hash conferido. Num build com a URL principal do FFmpeg quebrada de propósito (404), o download caiu na reserva e instalou o mesmo `ffmpeg.exe`. O teste `usa_a_reserva_quando_a_fonte_principal_falha` cobre a reserva, o arquivo adulterado, a pausa e a falha das duas fontes.
