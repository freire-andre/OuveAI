# ADR-008 — Substituição do Whisper pelo Nemotron 3.5 ASR Streaming 0.6b na comparação

**Status:** Rejeitado em 02/11/2026 (ver o fim do documento)  
**Data:** 2026-11-02  
**Escopo:** Parte 4 / Fase 7 — comparação opcional de trechos duvidosos  
**Modelo principal mantido:** Parakeet TDT 0.6b v3 int8  
**Modelo revisor proposto:** `nvidia/nemotron-3.5-asr-streaming-0.6b`  
**Modelo revisor removido da recomendação padrão:** Whisper large-v3 / large-v3-turbo

---

## 1. Decisão

Substituir o **Whisper** como modelo de segunda opinião na Parte 4 pelo **Nemotron 3.5 ASR Streaming 0.6b**.

O Parakeet TDT 0.6b v3 continua sendo o **único ASR responsável pela transcrição principal**, pelos timestamps, pela segmentação, pelas legendas e pela linha do tempo de `EU` / `OUTROS`.

O Nemotron passa a ser um **revisor local e opcional**, acionado apenas para trechos marcados como possivelmente incorretos ou quando o usuário pedir explicitamente uma comparação.

**Regra inegociável:** o texto final pode vir do Nemotron, mas **os tempos continuam sendo os do Parakeet**. O Nemotron não pode reescrever, deslocar, interpolar ou substituir `starts` e `ends` das palavras.

---

## 2. Motivação

A troca é recomendada porque o Nemotron é mais alinhado ao caso de uso do OuveAI:

- Declara suporte explícito a **pt-BR** e **pt-PT**, enquanto o Whisper trata português de forma mais genérica.
- É um modelo de 0,6B parâmetros, mais próximo em escala do Parakeet do que o Whisper large-v3 / large-v3-turbo.
- Foi projetado para streaming, com chunks configuráveis, o que o torna um candidato natural para futuras melhorias do modo ao vivo.
- Possui pontuação e capitalização nativas.
- Expõe timestamps em nível de token e palavra, úteis para alinhamento e diagnóstico.
- No FLEURS português, o modelo reporta WER de 5,48% com idioma informado e chunk de 1,12 s; em pt-BR/pt-PT, os valores reportados variam de 5,47% a 6,41% conforme o chunk.

A comparação com Whisper não deixa de ser tecnicamente possível, mas deixa de ser a recomendação padrão. O Whisper pode permanecer como **engine alternativo experimental**, apenas se já estiver implementado e se houver motivo concreto para mantê-lo.

---

## 3. Papel de cada modelo

| Papel | Modelo | Responsabilidade |
|---|---|---|
| ASR principal | Parakeet TDT 0.6b v3 int8 | Transcrever reuniões e arquivos; gerar palavras, timestamps, pausas, segmentos, `.txt`, `.srt`, `.json` e `.md` |
| VAD | Silero VAD | Delimitar fala e apoiar o pipeline ao vivo |
| Revisor de texto | Nemotron 3.5 ASR Streaming 0.6b | Gerar uma segunda hipótese para trechos duvidosos |
| Fonte da verdade temporal | Parakeet TDT 0.6b v3 int8 | Definir `starts` e `ends` de todas as palavras |
| Fonte da verdade do texto | Usuário | Escolher o texto de cada trecho divergente |

---

## 4. Mudanças no plano

### 4.1 Parte 4 / Fase 7

Substituir a redação atual da Parte 4 por:

> **Parte 4 — Comparação opcional com Nemotron 3.5 ASR Streaming 0.6b**
>
> A Parte 4 é um recurso opcional, sob pedido, para revisar trechos da transcrição principal.
>
> O modelo principal continua sendo o Parakeet TDT 0.6b v3 int8. O Nemotron 3.5 ASR Streaming 0.6b é usado apenas como segundo modelo de referência.
>
> O usuário pode acionar a comparação:
>
> - Em um trecho marcado como candidato a erro pelas heurísticas da Fase 3.
> - Em um intervalo selecionado manualmente na UI.
> - Pela CLI, informando um arquivo de áudio e um intervalo de tempo.
>
> A comparação roda somente com áudio salvo. Se o áudio da reunião não foi retido, o recurso não está disponível e a UI deve explicar o motivo.
>
> O `corecompare` executa os dois modelos no mesmo trecho de áudio, normaliza o texto e alinha as palavras por sequência similar. A UI mostra apenas as divergências relevantes, com contexto suficiente para decisão.
>
> O usuário escolhe o texto final por trecho. Os timestamps permanecem os do Parakeet.

### 4.2 Modelo padrão da comparação

No catálogo de modelos, criar uma entrada dedicada:

```text
id: nemotron-3.5-asr-streaming-0.6b
funcao: revisor
idioma_padrao: pt-BR
engine: sherpa-onnx
formato: ONNX
quantizacao: a definir no spike S8
tamanho: a medir no download real
sha256: a calcular no download real
url_principal: a fixar após validar o repositório oficial e o formato suportado
url_espelho: a definir se existir espelho confiável
estado_inicial: ausente
download: sempre com confirmação explícita
uso: apenas comparacao sob pedido
```

**Não usar valores de tamanho, SHA-256, URL ou nome exato de arquivo inventados.** O agente deve preencher esses campos somente depois de validar a página oficial do modelo, o repositório ou espelho escolhido e o artefato realmente baixado.

### 4.3 Remoção do Whisper como padrão

Remover das recomendações padrão:

- Whisper large-v3 como modelo de comparação.
- Whisper large-v3-turbo como modelo de comparação.
- A exigência de 3 GB de disco reservados para a Parte 4, salvo se o Whisper permanecer como opção experimental.

O plano deve passar a reservar espaço com base no modelo Nemotron realmente escolhido, medido no spike S8.

### 4.4 Nomenclatura

Substituir, em todo o plano e na UI:

| Termo antigo | Termo novo |
|---|---|
| “Comparação com Whisper” | “Comparação com segundo modelo” |
| “Parte 4 Whisper” | “Parte 4 — Comparação opcional” |
| “Whisper large-v3-turbo” | “Nemotron 3.5 ASR Streaming 0.6b” |
| “modelo Whisper” | “modelo revisor” |

Se o Whisper continuar implementado como opção experimental, a UI deve identificá-lo claramente como **experimental**, sem sugerir que é o padrão.

---

## 5. Comportamento do `corecompare`

### 5.1 Entrada

O `corecompare` recebe:

- O caminho do WAV já convertido para 16 kHz mono PCM16.
- O intervalo `[inicio, fim]` em segundos.
- A transcrição original do Parakeet para o intervalo.
- O identificador do modelo revisor.
- As opções de execução, incluindo número de threads.

### 5.2 Execução

1. Recortar ou alimentar ao modelo apenas o intervalo solicitado.
2. Executar o Nemotron com configuração determinística e registrada no resultado.
3. Obter texto, tokens, timestamps e metadados disponíveis.
4. Normalizar o texto dos dois modelos.
5. Alinhar as palavras por sequência similar.
6. Identificar divergências.
7. Gerar um relatório comparativo.
8. Aguardar decisão do usuário antes de alterar qualquer palavra.

### 5.3 Normalização obrigatória

Antes do alinhamento, aplicar normalização:

- Converter para minúsculas.
- Remover pontuação.
- Remover acentos apenas para comparação, sem alterar o texto exibido.
- Colapsar espaços repetidos.
- Tratar números por uma política definida: comparar a forma falada e a forma escrita como equivalentes quando representarem o mesmo valor.
- Não remover palavras de preenchimento automaticamente.

A normalização é usada apenas para decidir se há divergência. O texto exibido e o texto escolhido pelo usuário devem preservar a grafia original de cada modelo.

### 5.4 Saída

Gerar um objeto comparativo com, no mínimo:

```json
{
  "modelo_principal": "parakeet-tdt-0.6b-v3-int8",
  "modelo_revisor": "nemotron-3.5-asr-streaming-0.6b",
  "intervalo": {
    "inicio": 0.000,
    "fim": 20.000
  },
  "configuracao": {
    "threads": 4,
    "chunk": "valor_real_usado",
    "idioma": "pt-BR"
  },
  "parakeet": {
    "texto": "texto original",
    "palavras": []
  },
  "nemotron": {
    "texto": "texto alternativo",
    "palavras": []
  },
  "divergencias": [
    {
      "tipo": "substituicao|insercao|remocao|reordenacao",
      "inicio_parakeet": 0.000,
      "fim_parakeet": 0.000,
      "texto_parakeet": "",
      "texto_nemotron": "",
      "confianca_alinhamento": "alta|media|baixa"
    }
  ]
}
```

Campos que o runtime não expuser devem ser omitidos ou marcados explicitamente como indisponíveis. **Nunca inventar timestamps, confiança ou metadados.**

---

## 6. Regras de decisão do usuário

A UI deve permitir:

- Aceitar o texto do Parakeet.
- Aceitar o texto do Nemotron.
- Editar manualmente o texto do trecho.
- Descartar a sugestão do Nemotron.
- Ver o trecho antes e depois da divergência.
- Ver o áudio correspondente, se ele estiver salvo.
- Aplicar a escolha apenas ao trecho selecionado.

A aplicação de uma escolha deve:

- Alterar somente o campo `word` das palavras afetadas.
- Preservar os timestamps do Parakeet.
- Marcar as palavras alteradas como `editado`.
- Registrar a origem da alteração: `parakeet`, `nemotron` ou `manual`.
- Regenerar `.txt`, `.srt`, `.json` e `.md` a partir do banco, sem recalcular tempos.

Se o usuário escolher um texto do Nemotron com quantidade de palavras diferente do Parakeet, o app deve aplicar uma política explícita:

- **Preferência padrão:** manter o número e os limites temporais das palavras do Parakeet, redistribuindo o texto escolhido apenas quando o alinhamento for confiável.
- **Se o alinhamento for ambíguo:** aplicar o texto como alteração de segmento, mantendo o intervalo temporal do segmento original e marcando o trecho como editado.
- **Nunca:** criar timestamps novos por estimativa, interpolação ou divisão arbitrária.

---

## 7. Critérios de acionamento

A comparação pode ser sugerida automaticamente quando a Fase 3 identificar candidatos a erro, por exemplo:

- Grafias típicas de português europeu com equivalente em pt-BR, como “equipa/equipe”, “facto/fato”, “registo/registro”, “utilizador/usuário”, “ecrã/tela” e “telemóvel/celular”.
- Palavras com inicial maiúscula fora do início de frase, possivelmente nomes próprios.
- Tokens de baixa confiança, somente se o runtime expuser essa informação.
- Palavras curtas em inglês ou espanhol no meio de texto em português.
- Repetições anormais, texto vazio em trecho com fala detectada ou trechos com alta taxa de candidatos a erro.

A sugestão automática **não executa** o Nemotron sozinha. Ela apenas oferece o botão ou comando de comparação.

---

## 8. Spike obrigatório antes da implementação

Criar o spike **S8 — Nemotron como revisor** antes de alterar a Parte 4.

### 8.1 Objetivos

Validar, com o runtime e o artefato real:

1. Se o Nemotron 3.5 ASR Streaming 0.6b pode ser carregado pelo sherpa-onnx na versão usada pelo OuveAI.
2. Qual variante de arquivo é compatível: ONNX oficial, exportação suportada, INT8, INT4 ou outra quantização disponível.
3. Quais campos o wrapper Rust realmente expõe: texto, tokens, timestamps, durações, pontuação, capitalização e confiança.
4. Qual é o tamanho real do download e depois da extração.
5. Qual é o SHA-256 do artefato.
6. Qual é o tempo de carga do modelo.
7. Qual é o fator de tempo real em arquivo, usando a definição do plano: `duracao / tempo_de_transcricao`.
8. Qual é a RAM de pico.
9. Qual é o comportamento com pt-BR coloquial, nomes próprios, siglas, números, “tá”, “né”, muletas e sobreposição de fala.
10. Se a comparação Parakeet × Nemotron produz divergências úteis em áudio real de reunião.

### 8.2 Conjunto de teste

Usar no mínimo:

- 10 trechos curtos de 10 a 20 segundos.
- 5 trechos de reunião real com Teams, Meet ou Zoom, se houver áudio salvo.
- 5 trechos de arquivo importado, incluindo pelo menos um com ruído.
- Pelo menos 3 trechos com nomes próprios, siglas ou termos técnicos.
- Pelo menos 3 trechos com fala coloquial brasileira.
- Pelo menos 2 trechos com sobreposição ou eco conhecido.

Cada trecho deve ter uma transcrição de referência revisada manualmente.

### 8.3 Métricas mínimas

Para cada trecho, registrar:

- Texto do Parakeet.
- Texto do Nemotron.
- Texto de referência.
- WER relativo à referência, quando houver referência confiável.
- Número de divergências reais após normalização.
- Número de falsas divergências causadas apenas por pontuação, acento ou maiúscula.
- Tempo de carga.
- Tempo de transcrição.
- Fator de tempo real.
- RAM de pico.
- Qualidade do alinhamento.

### 8.4 Critério de Go

O Nemotron pode se tornar o revisor padrão se:

- Carregar e executar localmente no Windows x64.
- Produzir texto em pt-BR utilizável.
- Não degradar o app principal nem competir de forma perceptível com o Parakeet durante uma reunião.
- Ter timestamps ou, no mínimo, permitir comparação confiável por segmento.
- Reduzir ou manter o número de erros reais em relação ao Parakeet em pelo menos parte dos trechos difíceis.
- Não introduzir alucinações frequentes, repetições cíclicas ou texto em outro idioma.

### 8.5 Critério de No-Go

Manter o Whisper ou adiar a Parte 4 se:

- O artefato compatível com sherpa-onnx não existir ou não puder ser validado.
- O modelo não produzir timestamps utilizáveis e a comparação por segmento também não for confiável.
- O consumo de RAM ou CPU inviabilizar o uso em máquina com 8 GB de RAM.
- O modelo alucinar com frequência em pt-BR coloquial.
- O download oficial não puder ser validado por URL, tamanho e SHA-256.

---

## 9. Integração com o catálogo de modelos

O catálogo deve tratar o Nemotron como um modelo independente, com estado próprio:

```text
ausente -> baixando -> pausado -> verificando -> extraindo -> pronto -> corrompido
```

Requisitos:

- Download com confirmação explícita.
- Validação de tamanho e SHA-256.
- Retomada por `Range`.
- Suporte a download pelo navegador e importação de arquivo ou pasta.
- Armazenamento em `LOCALAPPDATA`, no mesmo padrão dos demais modelos.
- Nenhum download automático.
- Nenhuma chamada de rede durante a comparação.
- O modelo revisor nunca é pré-carregado junto com o Parakeet ao iniciar uma reunião.

---

## 10. Privacidade e limites

- O áudio nunca sai da máquina.
- A comparação é totalmente local.
- Nenhum texto transcrito pode ir para logs.
- Nenhum título de sessão pode ir para logs.
- O modelo revisor só pode rodar quando o usuário pedir ou aceitar explicitamente a sugestão.
- Se o áudio não foi salvo, a comparação não pode ser executada.
- A UI deve deixar claro que o Nemotron é um segundo modelo local, e não um serviço externo.

---

## 11. Riscos

| ID | Risco | Probabilidade | Impacto | Mitigação |
|---|---|---|---|---|
| N1 | O artefato oficial do Nemotron não for compatível com o sherpa-onnx usado pelo OuveAI | Média | Alto | Executar o spike S8 antes de qualquer implementação; validar formato, quantização e campos expostos |
| N2 | O wrapper Rust não expõe timestamps ou metadados necessários | Média | Alto | Comparar por segmento como fallback; nunca inventar timestamps; documentar a limitação |
| N3 | O modelo confunde pt-BR com pt-PT ou produz grafia europeia | Média | Médio | Fixar locale pt-BR quando suportado; heurísticas da Fase 3; editor de palavras; decisão humana por trecho |
| N4 | O modelo alucina em silêncio, ruído, música ou sobreposição | Média | Alto | Rodar apenas em trechos com fala validada pelo VAD; filtro de alucinação; exigir revisão humana |
| N5 | A comparação consome muita RAM ou CPU | Média | Médio | Executar somente sob pedido; limitar a trechos curtos; não pré-carregar durante reunião; medir no S8 |
| N6 | O alinhamento Parakeet × Nemotron é ambíguo | Alta | Médio | Normalização antes do alinhamento; confiança de alinhamento; política explícita para trechos ambíguos |
| N7 | O usuário assume que o texto do Nemotron é sempre melhor | Média | Médio | UI neutra; mostrar as duas hipóteses; não escolher automaticamente; registrar origem da alteração |
| N8 | O download é bloqueado por proxy corporativo | Alta | Médio | Usar o mesmo downloader do Parakeet: certificados do Windows, proxy do sistema, retomada, hash e importação local |
| N9 | O modelo muda de versão ou o repositório muda os artefatos | Média | Médio | Fixar versão, URL, tamanho e SHA-256 no catálogo; usar espelho somente se validado |
| N10 | A comparação é executada em reunião ao vivo e disputa CPU com o Teams | Média | Alto | Bloquear comparação durante gravação ativa; permitir somente com áudio salvo e fora da sessão ao vivo |

---

## 12. Resalvas importantes

1. **O Nemotron não substitui o Parakeet na v1.0.** Ele é revisor; o Parakeet continua responsável pela transcrição principal e pelos tempos.

2. **A comparação não é correção automática.** Nenhuma palavra pode ser alterada sem decisão do usuário.

3. **Não há garantia de que o Nemotron seja melhor que o Parakeet em todos os trechos.** A decisão deve ser baseada no conjunto de testes local e na revisão humana.

4. **Não usar o FLEURS como única evidência.** O benchmark é útil, mas o produto precisa ser validado com áudio real de reunião e arquivos do domínio do usuário.

5. **Não iniciar a Parte 4 antes da v1.0.** A troca afeta a Parte 4, não a prioridade atual de entregar reunião ao vivo, histórico, busca e piloto.

6. **Não remover o Whisper do código se ele já estiver funcional.** Ele pode permanecer como opção experimental, desmarcada por padrão, enquanto o Nemotron não passar no spike S8.

7. **Não prometer suporte a todos os formatos de modelo.** A compatibilidade depende do sherpa-onnx, da versão do runtime e do artefato realmente disponível.

8. **Não usar o Nemotron para gerar timestamps finais.** Mesmo que ele exponha timestamps, a linha do tempo oficial continua sendo a do Parakeet.

---

## 13. Tarefas para o agente executor

### Tarefa 1 — Atualizar documentação

- Renomear a Parte 4 para “Comparação opcional com segundo modelo”.
- Substituir Whisper large-v3 / large-v3-turbo por Nemotron 3.5 ASR Streaming 0.6b como recomendação padrão.
- Atualizar a tabela de ADRs para incluir o ADR-008.
- Atualizar referências a `corecompare`, `ouveai compare` e à Parte 4.

### Tarefa 2 — Criar spike S8

- Validar o repositório oficial do modelo.
- Identificar o artefato compatível com sherpa-onnx.
- Confirmar formato, quantização, tamanho, SHA-256 e licença.
- Rodar o modelo em pt-BR.
- Registrar campos disponíveis no wrapper Rust.
- Medir carga, RAM, tempo e fator de tempo real.
- Comparar com Parakeet em pelo menos 10 trechos reais.
- Produzir relatório `docs/fase0/s8-nemotron-revisor.md`.

### Tarefa 3 — Atualizar catálogo

- Adicionar entrada do Nemotron com função `revisor`.
- Não preencher hash, tamanho ou URL sem validação real.
- Garantir download confirmado, hash, retomada e importação local.
- Garantir que o modelo não seja baixado automaticamente.

### Tarefa 4 — Implementar `corecompare`

- Implementar execução do Parakeet e do Nemotron no mesmo intervalo.
- Implementar normalização e alinhamento.
- Implementar relatório de divergências.
- Implementar escolha manual do texto.
- Preservar timestamps do Parakeet.
- Marcar edições e registrar a origem.
- Regenerar exportações a partir do banco.

### Tarefa 5 — Ajustar UI

- Renomear a funcionalidade para “Comparação com segundo modelo”.
- Criar tela lado a lado.
- Mostrar apenas divergências relevantes.
- Permitir escolha por trecho.
- Bloquear comparação durante gravação ativa.
- Explicar quando o recurso não está disponível por falta de áudio salvo.

### Tarefa 6 — Ajustar CLI

- Manter `ouveai compare`.
- Adicionar parâmetro explícito para o modelo revisor.
- Permitir comparação por intervalo.
- Gerar relatório JSON legível.
- Não executar comparação automática sem solicitação.

### Tarefa 7 — Testes

- Testes unitários de normalização.
- Testes unitários de alinhamento.
- Testes unitários de aplicação de escolha sem alterar timestamps.
- Testes de replay com trechos conhecidos.
- Teste em máquina limpa Windows Sandbox.
- Teste com proxy corporativo ou importação local.
- Teste de RAM, CPU e tempo em máquina com 8 GB de RAM.

---

## 14. Critério de pronto

A troca pode ser considerada pronta quando:

- O spike S8 estiver concluído e documentado.
- O Nemotron estiver no catálogo com URL, tamanho, SHA-256 e licença validados.
- O `corecompare` rodar Parakeet e Nemotron no mesmo intervalo.
- A UI mostrar divergências e permitir escolha manual.
- Os timestamps permanecerem sempre do Parakeet.
- O usuário puder aceitar, rejeitar ou editar a sugestão.
- As exportações `.txt`, `.srt`, `.json` e `.md` forem regeneradas corretamente.
- Nenhum texto ou áudio for enviado para fora da máquina.
- A comparação não rodar automaticamente durante uma reunião ao vivo.
- O Whisper não for mais o padrão, permanecendo opcional apenas se já estiver implementado.

---

## 15. Recomendação final

**Adotar o Nemotron 3.5 ASR Streaming 0.6b como revisor padrão da Parte 4**, substituindo o Whisper large-v3-turbo.

**Não adotá-lo como ASR principal nesta etapa.** O Parakeet TDT 0.6b v3 int8 permanece como motor principal por já estar integrado, validado no fluxo do OuveAI e responsável pelos timestamps usados em todo o produto.

A troca deve ser implementada somente após o **spike S8**, com validação real de compatibilidade, artefato, licença, RAM, CPU, timestamps e qualidade em pt-BR.

---

## Decisão final (02/11/2026): rejeitado

O spike [S8](fase0/s8-nemotron-revisor.md) testou o Nemotron 3.5 ASR Streaming 0.6b no sherpa-onnx, com as mesmas janelas do app (cortadas no silêncio), em um vídeo narrado e em uma conversa a duas vozes. Resultado:

- O **Whisper large-v3-turbo foi mais preciso** como revisor. No vídeo narrado, 5 sugestões certas e nenhuma errada, contra 5 certas e 3 erradas do Nemotron na melhor configuração (stream contínuo, 1,5 s de silêncio inicial e corte por confiança).
- Com `pt-BR`, o Nemotron sai sem pontuação e com números por extenso. Hotwords, beam search e `blank_penalty` não melhoraram o resultado.
- A recomendação anterior a favor do Nemotron veio de um erro de método: janelas fixas de 20 s, que prejudicavam o Whisper.

**Decisão:** o Whisper continua como o único revisor opcional (ADR-006), com os ajustes de comparação feitos durante o spike: número por extenso igual a dígito, repetição imediata ignorada, pontuação e maiúscula do Parakeet mantidas ao aplicar, e frase inteira que só o Parakeet ouviu sem virar sugestão de apagar. O Nemotron não entra nem como experimental, e o código do spike não ficou no repositório.

O ganho em português veio pelo outro lado: a troca do Parakeet multilíngue pelo ajuste pt-BR ([ADR-009](adr/009-parakeet-ptbr.md)).

