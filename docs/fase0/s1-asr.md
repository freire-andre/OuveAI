# Spike S1 · ASR (Parakeet TDT 0.6b v3 int8 via crate `sherpa-onnx`)

> 30/10/2026 · `crates/ouveai-core/examples/s1_asr.rs` · build release, link estático (padrão do crate)

## Ambiente

- Crate `sherpa-onnx` **2.0.1** (+ `sherpa-onnx-sys` 2.0.1). O build baixa sozinho a lib nativa `sherpa-onnx-v2.0.1-win-x64-static-MT-Release-lib.tar.bz2` (~117 MB) para `target/sherpa-onnx-prebuilt` (variáveis `SHERPA_ONNX_LIB_DIR` / `SHERPA_ONNX_ARCHIVE_DIR` permitem usar uma cópia local).
- Áudio: gravação própria em pt-BR, 123,97 s (m4a 48 kHz estéreo → WAV 16 kHz mono PCM16 via FFmpeg), com vários nomes próprios e marcas.
- Máquina: Ryzen 5 5600X, `threads = min(4, núcleos) = 4`, AVX2 detectado.

## Resultado — **Go** ✅ (há timestamps e durations)

| Medida | Valor |
|---|---|
| Carga do modelo | **3,26 s** (RAM pico após carga: 724 MB) |
| Transcrição (124 s de áudio, greedy) | **11,74 s** |
| Fator de tempo real (duração ÷ tempo) | **10,6× mais rápido que o tempo real** |
| RAM pico (áudio inteiro de 2 min num só stream) | **1.643 MB** |

## API real (regra 5)

```rust
pub struct OfflineRecognizerResult {
    pub text: String,
    pub tokens: Vec<String>,
    pub timestamps: Option<Vec<f32>>,   // início de cada token, em s
    pub durations: Option<Vec<f32>>,    // duração de cada token, em s
}
```
- Config: `model_config.transducer {encoder, decoder, joiner}`, `model_config.tokens`, `model_config.model_type = "nemo_transducer"`, `num_threads`, `provider = "cpu"`, `decoding_method`.
- Os três vetores vieram com o mesmo tamanho (351). **`durations` presente.**
- O wrapper só desserializa esses 4 campos do JSON do C; **não há confiança por token** exposta → a heurística 3 de "candidatos a erro" (Fase 3) fica sem fonte, a não ser via `sherpa-onnx-sys` direto (avaliar na Fase 3).

## Achados que afetam o plano

1. **Marcador de palavra = espaço inicial**, não `▁` (0 tokens com `▁`, 196 com espaço). `core::words` deve tratar os dois, como a regra 2 já prevê.
2. **Resolução de 80 ms:** timestamps e durations são múltiplos de 0,08 s (frame do encoder).
3. **Pontuação tem timestamp próprio, depois da pausa.** Ex.: `"ion"` em 2,400 s (dur 0,08) e `","` em 4,160 s. Pela regra 2 literal (fim = início + duração do *último token*), a palavra "reunion," terminaria em 4,24 s em vez de 2,48 s, engolindo 1,8 s de silêncio. **Proposta:** fim = início + duração do último token *que não seja só pontuação*. ⚠️ Ajuste na regra 2 — precisa do seu OK.
4. **Troca de idioma (code-switching)** — o Parakeet v3 não tem como fixar o idioma, e em pt-BR escorrega para espanhol/inglês em palavras curtas: "reunion" (2×), "but", "and", "or", "automations". Isso é mais frequente que as grafias de pt-PT previstas no R13. **Proposta:** acrescentar às heurísticas de "candidatos a erro" uma lista de palavras curtas de en/es (but, and, or, the, of, for, reunion, …) e, no backlog, um dicionário de correção do usuário.
5. **Nomes próprios:** a maioria dos nomes de pessoas, tecnologias e marcas da gravação saiu certa ✅. Os erros foram em marcas pouco conhecidas e trechos incompreensíveis.
6. **Hotwords não funcionam com este modelo.** `modified_beam_search` cria o reconhecedor, mas `Encode hotwords failed, skipping` (o pacote não traz `bpe.vocab` para `modeling_unit=bpe`). Além disso, o beam search **omitiu duas frases inteiras** que o greedy pegou. → Usar **greedy_search**; hotwords para o backlog (exige gerar o vocabulário BPE).
7. **RAM:** 1,6 GB com 2 min num só stream. Ao vivo os segmentos têm ≤ 20 s, então a meta de < 1,5 GB é plausível — medir na Fase 4. Na Fase 3 o corte em pedaços de ~10 min deve ser reavaliado se a RAM crescer com o tamanho do pedaço.
8. **Link:** o build emite `LNK4098: defaultlib 'LIBCMT' conflita…` — a lib estática do sherpa é `/MT` e o Rust usa `/MD` por padrão. Funciona, mas é mistura de CRT. Investigar no S2 (`+crt-static`).

Saída bruta: `testdata/ptbr_01.s1.json` (não versionada).
