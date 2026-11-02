# Ajustes e Fine-tuning do Parakeet TDT 0.6B v3 para pt-BR

> Objetivo: orientar um agente de IA a configurar, avaliar e aplicar fine-tuning no `nvidia/parakeet-tdt-0.6b-v3` para obter a melhor transcrição em português brasileiro.

## 1. Diagnóstico inicial

O `parakeet-tdt-0.6b-v3` original é multilíngue, mas **não aceita prompt de idioma**. Ele tenta detectar o idioma automaticamente, e essa detecção pode falhar em trechos curtos de áudio em português, ocasionalmente produzindo texto em inglês. [web:22]

A NVIDIA também observa que parte do treinamento usa português europeu, enquanto a maior parte dos benchmarks e do uso prático no Brasil envolve português brasileiro; portanto, o checkpoint base pode não ser ótimo para pt-BR sem adaptação. [web:12][web:20]

A precisão varia conforme domínio, sotaque, ruído, tipo de fala e contexto. [web:17][web:20]

## 2. Decisão principal: usar base pt-BR

**Antes de treinar do zero ou ajustar hiperparâmetros, o agente deve avaliar um checkpoint já fine-tunado para pt-BR.**

Motivos identificados:

- Um fine-tune exclusivo para pt-BR elimina o risco de o modelo “derivar” para inglês, porque o modelo passa a ser especializado em português. [web:21][web:22]
- Variantes da comunidade preservaram o tokenizer SentencePiece BPE do v3 original durante o fine-tune para pt-BR, o que reduz riscos de incompatibilidade de vocabulário. [web:16]
- Em um teste indicativo de 90 segundos em pt-BR, um fine-tune pt-BR obteve WER de 12,7%, contra 13,3% do modelo original; o ganho principal relatado foi confiabilidade de idioma, não apenas WER. O teste é de gravação única e não deve ser tratado como benchmark definitivo. [web:22]

### Ordem de avaliação

1. `nvidia/parakeet-tdt-0.6b-v3` — baseline multilíngue.
2. Checkpoint pt-BR fine-tunado disponível na comunidade.
3. Fine-tuning adicional do melhor checkpoint com dados do domínio do usuário.

## 3. Ajustes de inferência

### Áudio

- Converter para **16 kHz, mono, PCM/WAV** antes da inferência.
- Normalizar volume e remover cliques, eco, silêncios excessivos e ruído forte.
- Evitar cortar palavras no início ou fim dos segmentos.
- Para telefonias, aplicar filtragem de ruído e de-reverberação apenas se melhorarem o WER no conjunto de teste.

### Segmentação

- Para gravações longas, usar janelas de aproximadamente **30 segundos**.
- Manter contexto futuro/lookahead; uma configuração documentada usa `right_context_sec = 2`. [web:9]
- Ativar modo *long-form* para áudio acima de 30 segundos, preservando timestamps e estado do decodificador. [web:9]
- Não enviar trechos muito curtos, especialmente abaixo de poucos segundos, pois aumentam o risco de erro de detecção de idioma no modelo multilíngue. [web:22]

### Precisão numérica

| Ambiente | Recomendação |
|---|---|
| GPU | FP32 ou BF16/FP16; priorizar qualidade. Um deployment ONNX para pt-BR recomenda FP32 em GPU. [web:1] |
| CPU | BF16 quando suportado; comparar WER antes de usar quantização mais agressiva. [web:9] |
| Edge/CPU com restrição | INT8/ONNX apenas após validar perda de qualidade. [web:1][web:16] |

### Pós-processamento

O Parakeet calcula WER removendo pontuação e capitalização, então esses aspectos não são o foco central do modelo. [web:10]

Aplicar pós-processamento separado para:

- Pontuação e capitalização.
- Números, datas, moedas e unidades.
- Normalização de siglas: CPF, CNPJ, PIX, RG, CEP etc.
- Correção de nomes próprios, produtos, medicamentos, códigos internos e jargão do domínio.
- Detecção e correção de trechos em inglês, caso o modelo base ainda seja usado.

## 4. Métricas e conjunto de teste

Criar um conjunto de validação **antes** de qualquer fine-tuning.

### Composição sugerida

- 100 a 500 clipes reais do caso de uso.
- Duração variada: 5 s, 15 s, 30 s, 60 s e acima de 60 s.
- Diversidade de sotaques e regiões do Brasil.
- Áudio limpo, telefonias, reuniões, ruído moderado e fala espontânea.
- Transcrições humanas revisadas.

### Métricas

- **WER**: métrica principal.
- **CER**: útil para nomes, códigos, números e erros de grafia.
- **Taxa de alucinação/inserção**: especialmente importante em silêncio, ruído e áudio curto.
- **Taxa de drift para inglês**: percentual de trechos pt-BR transcritos em inglês.
- **WER por domínio**: separar resultados por tipo de áudio.
- **Latência e RTF**: tempo real de processamento.

## 5. Preparação do dataset de fine-tuning

### Formato mínimo

Criar um manifest JSONL no formato esperado pelo NeMo:

```json
{"audio_filepath": "data/audio/0001.wav", "duration": 12.4, "text": "Texto exatamente como deve ser transcrito."}
```

### Regras de transcrição

- Usar português brasileiro consistente.
- Definir uma política única para números: por exemplo, sempre dígitos ou sempre por extenso.
- Definir política para siglas, unidades, moedas, datas e horários.
- Remover marcações de ruído, risadas e eventos não-fala, salvo se o objetivo for treinar o modelo para ignorá-los explicitamente.
- Não incluir transcrições automáticas não revisadas como referência.
- Verificar duração declarada no manifest contra a duração real do arquivo.

### Quantidade recomendada

| Cenário | Volume inicial |
|---|---|
| Domínio estreito, vocabulário específico | 5 a 20 horas |
| Melhoria geral de pt-BR | 20 a 100 horas |
| Produção robusta com vários sotaques e ruído | 100+ horas |

Qualidade, diversidade e consistência importam mais do que apenas aumentar horas de áudio. O fine-tuning público de referência usou cerca de 7.500 horas de dados humanos transcritos em 5.000 passos em 4 GPUs A100, mas isso é um cenário de treinamento em larga escala, não o mínimo necessário para adaptação de domínio. [web:14][web:15]

## 6. Estratégia de fine-tuning

### Estratégia recomendada

1. Partir de um checkpoint pt-BR fine-tunado, se disponível.
2. Fazer fine-tuning supervisionado com dados do domínio.
3. Congelar parcialmente o encoder nas primeiras épocas se o dataset for pequeno.
4. Avaliar após cada época.
5. Manter o checkpoint com melhor WER/CER no conjunto de validação, não necessariamente o último.

### Hiperparâmetros iniciais

| Parâmetro | Valor inicial sugerido |
|---|---|
| Learning rate | 1e-5 a 3e-5 |
| Batch size efetivo | 16 a 64, conforme VRAM |
| Épocas | 3 a 10 |
| Warmup | 5% a 10% dos passos |
| Weight decay | 1e-2 a 1e-4 |
| Early stopping | Baseado em WER de validação |
| Precisão | BF16 em GPU, se estável |

### Salvaguardas

- Não alterar o tokenizer sem necessidade; um fine-tune pt-BR da comunidade preservou o tokenizer BPE original. [web:16]
- Se o dataset for menor que 20 horas, usar learning rate baixo e evitar muitas épocas.
- Misturar 10% a 20% de dados gerais de pt-BR com os dados do domínio para reduzir esquecimento catastrófico.
- Registrar WER antes e depois do fine-tuning.
- Testar explicitamente áudio curto para verificar se o modelo continua emitindo português.

## 7. Pipeline sugerido para o agente

```text
1. Instalar PyTorch + NVIDIA NeMo ASR.
2. Baixar checkpoint base e checkpoint pt-BR.
3. Preparar conjunto de teste humano revisado.
4. Rodar baseline e medir WER, CER, latência e drift para inglês.
5. Selecionar o melhor checkpoint.
6. Preparar dataset de fine-tuning em JSONL.
7. Rodar fine-tuning com validação por época.
8. Comparar baseline vs. modelo ajustado.
9. Exportar o melhor modelo.
10. Aplicar pós-processamento de pontuação, números e vocabulário.
11. Monitorar WER em produção e coletar amostras de erro.
```

A documentação oficial do Parakeet indica o NVIDIA NeMo como ferramenta para inferência, treinamento e fine-tuning. [web:13][web:15][web:18]

## 8. Critérios de aceite

Considerar o ajuste bem-sucedido somente se:

- WER no conjunto de teste cair em relação ao baseline.
- Não houver aumento relevante de alucinações em silêncio ou ruído.
- Não houver drift para inglês em áudio pt-BR.
- Nomes, siglas e termos do domínio melhorarem ou permanecerem estáveis.
- Latência permanecer dentro do requisito do produto.
- O modelo generalizar para gravações novas, e não apenas memorizar o dataset de treino.

## 9. Riscos

- **Drift de idioma:** mais provável no modelo multilíngue original e em clipes curtos. [web:22]
- **Diferença pt-PT vs. pt-BR:** o treinamento original inclui português europeu, o que pode prejudicar resultados brasileiros. [web:12][web:20]
- **Quantização:** pode reduzir custo e latência, mas deve ser validada com WER/CER antes de produção. [web:1][web:16]
- **Overfitting:** comum com datasets pequenos ou repetitivos; usar validação separada e early stopping.
- **Pós-processamento incorreto:** correções automáticas podem introduzir erros; manter log e permitir revisão humana em casos críticos.
