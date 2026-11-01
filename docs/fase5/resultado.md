# Fase 5: histórico, busca e utilidades de reunião

Data: 01/11/2026 · Ryzen 5 5600X · Windows 11 build 26300 · Sandbox build 26100

## O que entrou

- **Busca sem diferenciar acento** (FTS5 `unicode61 remove_diacritics 2`)
  - Fica no topo de **Sessões** e busca nas reuniões e nos arquivos ao mesmo tempo.
  - Cada termo vale como prefixo e todos precisam aparecer. Pontuação e aspas não quebram a consulta.
  - O resultado mostra o trecho com o termo destacado, agrupado por sessão.
  - Clicar abre a reunião já na fala, destacada, ou abre o arquivo no editor.
  - Os ecos ficam de fora.
  - Triggers mantêm o índice em dia: fala nova, correção no editor e exclusão, inclusive pelo cascade.
  - A migração reconstrói o índice dos bancos que já tinham falas.
- **Copiar transcrição** no formato `**[00:12:03] OUTROS:** texto`:
  - falas seguidas do mesmo canal viram um bloco só;
  - as marcas ⭐ entram; os ecos ficam de fora;
  - a opção **Hora do relógio** troca `[00:12:03]` por `[14:32:05]`.
  - A área de transferência recebe Markdown como texto puro e o mesmo conteúdo em HTML, com negrito, que é o que o Teams e o Outlook usam ao colar.
- **Exportar** `.md` (com título e data), `.txt`, `.srt` e `.json`:
  - `.srt`: cada fala tem o canal na frente e é quebrada pelas regras da legenda de arquivo;
  - `.json`: tudo, inclusive os ecos marcados e as palavras com tempo.
- **Ouvir o trecho:** clicar no horário de uma fala toca o áudio salvo, EU e OUTROS misturados, até 60 s.
- **Dados neste computador** (Configurações):
  - quantidade de reuniões e arquivos;
  - espaço usado pelo áudio, pelo banco e pelos modelos;
  - "Abrir pasta dos dados";
  - **"Excluir tudo"**, com confirmação em dois passos. É recusado durante uma gravação ou importação.
- **Validade do áudio (D2)**
  - A limpeza roda ao abrir o app e depois de hora em hora.
  - A pasta do áudio é registrada já no início da gravação. Se o app cair, a sessão interrompida continua com o áudio, e ele vence como os outros.
- **Sessão interrompida:** além do texto, ganha a duração (o fim da última fala) e a validade do áudio.
- **ADR-004:** dados locais, validade e exclusão.

## Defeitos da Fase 4 encontrados aqui (e corrigidos)

O teste de "matar o processo" exigia fala de verdade no Sandbox. Toquei um WAV (`SoundPlayer`) na saída "Áudio Remoto", e isso revelou dois defeitos da captura que o teste da Fase 4, sem nada tocando, não pegava:

1. **O app fechava 2–3 s depois de começar a fala.**
   - Erro: `memory allocation of 7370054358964 bytes failed`. A CLI `capture` também caía.
   - Causa: o "Áudio Remoto" (Sandbox e, provavelmente, a Área de Trabalho Remota) entrega um tempo de pacote que não é QPC, 115 milhões de segundos à frente. A linha do tempo tentava reservar memória para essa "lacuna".
   - Correção: um pacote que diz vir do futuro segue pela contagem de amostras (log de aviso, uma vez por fonte), e a linha do tempo nunca reserva uma lacuna maior que 10 min.
2. **Sem a queda, nenhuma fala era transcrita** quando a gravação começava em silêncio.
   - A fonte sem relógio continuava de onde tinha parado. Enquanto isso, o resto da linha do tempo já tinha entregado esse trecho como silêncio, então tudo era descartado como "atrasado". O medidor mexia, mas o motor só recebia zeros.
   - Correção: a fonte retoma no primeiro ponto ainda não entregue.

Os dois casos têm teste em `ouveai-audio::timeline`, com o valor exato que derrubou o app. Em placas de som comuns o tempo vem certo e nada muda. Para quem usar o OuveAI por RDP, esses defeitos deixariam o app inutilizável.

## Verificação

**Testes automáticos:** 42 no core (eram 36) e 6 no `ouveai-audio` (eram 4). Novos:
- busca sem acento, por prefixo, sem eco, com a pontuação tratada, o índice atualizado na correção e limpo na exclusão;
- reconstrução do índice na migração;
- Markdown agrupado com ⭐ e sem eco;
- hora do relógio virando o dia;
- HTML escapado, `.txt`, `.srt` e `.json`;
- recorte, mistura e WAV, inclusive com o cabeçalho desatualizado de um app que caiu;
- sessão órfã com duração e validade;
- os dois casos da linha do tempo.

`clippy` limpo no código novo. `svelte-check` sem erros.

**Windows Sandbox** (instalador NSIS, sem rede, sem microfone, fala tocando no "Áudio Remoto"):

| Passo | Resultado |
|---|---|
| Gravar 45 s e **matar o processo** | Reaberto: "Reunião · 36 s · 68 palavras · interrompida", com o texto e o áudio válido até 08/11 |
| Reunião completa de 67 s | 4 falas, 113 palavras. Atraso p50 2,45 s, p95 3,47 s |
| Copiar transcrição | Texto `**[00:00:03] OUTROS:** Vamos iniciar essa reunião…` e HTML com `<b>` |
| Buscar `reuniao` | 4 resultados em 2 reuniões, com "reunião" destacado. O clique abre a fala certa |
| Dados neste computador | 2 reuniões, 7 MB de áudio, 640 MB de modelos |
| Excluir tudo | 0 reuniões, áudio "nada", `recordings\` vazia |
| Log (regra 8) | Só contagens, tempos e nomes de dispositivo |

**Limite do Sandbox:** tocar áudio pelo WebView2 derruba a VM inteira (erro 0x80370106). Reproduzi isso só com o Edge tocando um WAV, sem o OuveAI. Por isso o som do "ouvir o trecho" não foi testado no Sandbox. A leitura, a mistura e a geração do WAV têm teste automático, e ouvir de verdade fica para a validação na sua máquina.

## Validação do usuário (01/11/2026): ok

Validado pelo usuário na máquina dele:
- a transcrição copiada colou com negrito no chat do Teams;
- clicar no horário toca o trecho certo;
- o `.srt` exportado abre no VLC;
- a busca sem acento abre a fala certa.

Com isso, as Fases 4 e 5 estão fechadas.
