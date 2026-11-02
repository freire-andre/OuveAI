# ADR-007: Legenda de vídeo editado (Parte 5)

- **Status:** aceito (Fase 8), em 02/11/2026
- **Relacionados:** Parte 5 do plano, regra de tempo do prompt.txt, ADR-006 (alinhamento com `similar`)

## Decisões

1. **Formato do `edl.json`, igual ao prompt:** `[{"file", "in", "out", "position"}]`, em segundos e sem mudança de velocidade.
   - Caminho relativo em `file` é relativo à pasta do `edl.json`.
   - O `.json` de mesmo nome-base do bruto é obrigatório. Se faltar, o erro diz qual arquivo transcrever.
   - Os cortes são numerados a partir de 1, na ordem do arquivo.
2. **Validação antes de tudo.**
   - É erro: `out ≤ in`, valor negativo ou não numérico, sobreposição no vídeo final.
   - Lacunas entre cortes são permitidas (títulos, transições) e listadas uma vez, inclusive uma lacuna no começo.
   - Corte que passa do fim do bruto, ou que não tem nenhuma palavra, gera só aviso.
3. **Mapeamento, como no prompt.** Entra a palavra cujo início está em `[in, out)`. O tempo final é `t − in + position`, e um fim que passa de `out` é cortado em `out`. As pausas medidas entram recortadas ao corte, e a quebra por pausa continua valendo.
4. **Nenhum bloco atravessa um corte.**
   - A mesma `subs::segment` roda em cada corte.
   - O último bloco de um corte termina no fim do corte e respeita o intervalo mínimo antes do primeiro bloco do corte seguinte.
5. **Nome da legenda.** Sem o vídeo final, a legenda leva o nome-base do `edl.json`. Na verificação, ela leva o nome do vídeo final, para o player carregar sozinho.
6. **Verificação.**
   - O vídeo final é transcrito como no `--check` e gera um `.check.json`, que não toca em nenhum `.srt`. Esse `.check.json` é reaproveitado enquanto for mais novo que o vídeo.
   - O alinhamento usa `similar` com texto normalizado: minúsculas, sem pontuação nem acento. Só os pares casados entram na conta.
   - O relatório traz o desvio mediano e o máximo dos inícios, e as palavras sem par dos dois lados.
7. **Causa provável, por corte**, para os trechos com desvio maior que 0,3 s:
   - **Corte deslocado:** desvios parecidos em todo o corte (espalhamento ≤ 0,15 s, pelo menos 3 pares). O deslocamento proposto desconta o desvio de base dos cortes sem problema.
   - **Deslize:** o desvio muda ao longo do corte. Indica mudança de velocidade na edição ou bruto diferente, e não há correção automática.
   - **Palavras soltas:** reconhecimento diferente ou pareamento errado.
8. **Correção na origem, nunca nos tempos.**
   - Para um corte deslocado, a proposta é mudar `position`.
   - Se o novo `position` sobrepuser o vizinho por menos de um quadro do Parakeet (0,08 s, a incerteza da medição), o corte fica encostado nele. Se sobrepuser mais, a proposta é mudar `in` e `out` juntos.
   - A proposta só é aplicada com o clique da pessoa ("Aplicar no edl.json"). O app muda só os campos indicados, guarda a cópia `edl.json.bak` e confere que o valor antigo ainda é o da verificação.
   - Depois, a legenda e a verificação são refeitas sem transcrever de novo.
   - A CLI só imprime a proposta.
9. **Recursos:** a verificação carrega o Parakeet. Ela não começa com gravação, comparação ou importação em andamento, e a fila de arquivos espera por ela.

## Consequências

- **Precisão:** o deslocamento proposto tem a incerteza de um quadro do modelo (±0,04 s).
- **Unidades de palavra:** um trecho trocado pelo Whisper com número diferente de palavras é uma unidade só (ADR-006). Por isso, ele pode ficar sem par na verificação.
