# Fase 8: legenda de vídeo editado (Parte 5)

Data: 02/11/2026 · decisões em [ADR-007](../adr/007-legenda-edl.md)

## O que entrou

- **`core::edl`**
  - Lê e valida o `edl.json`: `out > in`, nenhuma sobreposição no vídeo final e lacunas listadas.
  - Exige o `.json` de cada bruto, com erro claro se faltar.
  - Mapeia palavras e pausas para o vídeo final e gera a legenda com a mesma `segment()`, sem bloco atravessando corte.
  - Verifica contra a transcrição do vídeo final.
- **Verificação**
  - Alinhamento por sequência com texto normalizado.
  - Desvio mediano e máximo e palavras sem par.
  - Trechos acima de 0,3 s com a causa provável: corte deslocado, deslize ou palavras soltas.
  - Correção proposta no `edl.json`.
- **CLI:** `ouveai-cli edl-subs <edl.json> [--out final.srt] [--check <vídeo final | final.check.json>]`.
- **Tela "Legendas"** (novo item no menu)
  - Mostra o formato do `edl.json`, a tabela de cortes (com as palavras que entram em cada um), os erros, os avisos e as lacunas.
  - Botões "Gerar legenda" e "Verificar com o vídeo final…".
  - O resultado aparece em números, com a lista de trechos e a correção proposta, que só é gravada com "Aplicar no edl.json" (cópia em `edl.json.bak`).
- **Fase 7, corrigido:** ao aplicar o Whisper, só tempos medidos. Com número diferente de palavras, o trecho vira uma unidade com o início e o fim medidos, sem fronteira interpolada (regra de tempo do prompt).

## Medições (CLI, áudio real)

Dois brutos de 90 s, cortados de uma gravação de teste e transcritos (300 e 330 palavras). Vídeo final de 82 s montado com o FFmpeg: 3 cortes, um deles do mesmo bruto, e uma lacuna de 2 s.

| Caso | Resultado |
|---|---|
| `edl.json` certo | **Desvio mediano 0,030 s** (referência: 0,12 s), máximo 0,59 s; 249 pares, 19 e 15 palavras sem par; 3 palavras soltas listadas, sem correção |
| `position` do corte 2 errado em 0,4 s (31,6 em vez de 32) | O corte 2 inteiro foi listado ("a fala aparece 0,44 s mais tarde… position errada"); proposta: **`position 31.600 → 32.000`** |
| Transcrição do vídeo final | 82 s em 6,4 s; reaproveitada nas verificações seguintes |

Os 0,04 s entre o erro real (0,40 s) e o medido (0,44 s) são a resolução do Parakeet (quadros de 0,08 s). Neste caso, a proposta encostou o corte no vizinho e chegou ao valor exato.

## Verificação

**Testes automáticos:** 71 no core (eram 59).
- **EDL (12 testes):** formato, validação, mapeamento `[in, out)` com o fim cortado, nenhum bloco atravessando corte, `.json` faltando, verificação sem desvio, corte deslocado, encosto no vizinho, troca de in/out, deslize, palavra solta e aplicação da correção (com `.bak`, valor conferido e recusa de sobreposição).
- **Fase 7:** os testes de tempo medido foram atualizados.

O `clippy` não aponta nada no código das fases (só os 3 avisos antigos do exemplo de captura), e o `svelte-check` não tem erros.

**Windows Sandbox** (instalador com a Fase 8, sem rede):

| Passo | Resultado |
|---|---|
| Legendas › Escolher o edl.json | Tabela de cortes e lacunas (0:30–0:31, 1:01–1:02) |
| Gerar legenda | `edl.srt` gravado, 53 blocos, 268 palavras |
| Verificar com o vídeo final | Reaproveitou o `final.check.json`, pronto em 8 s; desvio mediano 0,030 s; `final.srt` gravado |
| Aplicar no edl.json | `edl.json.bak` criado; `position` do corte 2 = 32.0; a proposta sumiu; legenda e verificação refeitas |
| RAM (sem modelo carregado) | 399 MB |
| Log do app | Só contagens e tempos |

**Sandbox × WebView2:** hoje a janela principal não abria na Sandbox, nem com o instalador da Fase 7, que funcionou ontem. O erro era "Could not find the webview runtime". O registro da Sandbox apontava para o WebView2 156.0.4314.50, mas a pasta instalada era a .58, uma inconsistência da imagem da Sandbox depois de uma atualização. O teste rodou apontando a pasta com `WEBVIEW2_BROWSER_EXECUTABLE_FOLDER`. O app não mudou nada nisso.

## Limites conhecidos

- **Precisão:** o deslocamento proposto tem a incerteza de um quadro do modelo (±0,04 s).
- **Deslize** (mudança de velocidade) é apontado, mas não tem correção automática.
- **Ambiguidade da causa:** a verificação não distingue `position` errado de `in` e `out` deslocados juntos (o efeito no tempo é o mesmo). A proposta prefere `position`.

## Depois da fase: aviso de WebView2 em português

Sem o WebView2 (ou com ele quebrado), nenhuma tela do app abre. O Tauri já mostrava uma caixa genérica em inglês ("Could not find the WebView2 Runtime"), e só depois de cerca de 54 s.

Agora o app checa o WebView2 antes de abrir qualquer janela (principal, assistente e mini). Se ele faltar, a janela não é pedida e aparece na hora uma caixa nativa do Windows em português:
- explica o que é o WebView2;
- diz como resolver: instalar ou reparar o "Microsoft Edge WebView2 Runtime", ou pedir ao TI;
- avisa que nada foi perdido.

Outras falhas ao criar uma janela também mostram uma caixa, com o detalhe e o caminho do log. As caixas não se empilham.

Na Sandbox (que hoje está com o WebView2 inconsistente), a caixa apareceu em 0 s, uma só para dois cliques seguidos, sem a caixa em inglês, e voltou depois de fechada. Com o WebView2 apontado, a janela abriu normalmente.
