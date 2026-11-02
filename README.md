<div align="center">

<img src="image/OuveAI.png" alt="OuveAI: transcrição de reuniões no seu computador" width="100%">

<h1>OuveAI</h1>

<p><b>Transcrição de reuniões e de arquivos, 100% local, no Windows.</b><br>
O áudio não sai do seu computador: sem nuvem, sem conta, sem telemetria.</p>

<p>
  <img alt="Windows 10/11" src="https://img.shields.io/badge/Windows-10%20%7C%2011-0078D4?logo=windows&logoColor=white">
  <img alt="Tauri v2" src="https://img.shields.io/badge/Tauri-v2-24C8DB?logo=tauri&logoColor=white">
  <img alt="Rust" src="https://img.shields.io/badge/Rust-stable-000000?logo=rust&logoColor=white">
  <img alt="Svelte 5" src="https://img.shields.io/badge/Svelte-5-FF3E00?logo=svelte&logoColor=white">
  <img alt="Parakeet TDT v3 pt-BR" src="https://img.shields.io/badge/ASR-Parakeet%20TDT%20v3%20pt--BR-1F6FEB">
  <img alt="Status" src="https://img.shields.io/badge/status-piloto-F2A600">
</p>

<p>
  <a href="https://github.com/freire-andre/OuveAI/releases/latest">
    <img alt="Baixar para Windows" src="https://img.shields.io/badge/Baixar_para_Windows-instalador_.exe-2EA44F?style=for-the-badge&logo=windows&logoColor=white" height="44">
  </a>
  <br>
  <a href="https://github.com/freire-andre/OuveAI/releases/latest">
    <img alt="Última versão" src="https://img.shields.io/github/v/release/freire-andre/OuveAI?label=vers%C3%A3o&color=2EA44F">
  </a>
  <img alt="Downloads" src="https://img.shields.io/github/downloads/freire-andre/OuveAI/total?label=downloads&color=555">
  <br>
  <sub>Windows 10 (2004+) ou 11 · 64 bits · sem administrador · <a href="docs/guia.md">guia rápido</a></sub>
</p>

<p>
  <a href="#-o-que-ele-faz">O que faz</a> ·
  <a href="#-privacidade">Privacidade</a> ·
  <a href="#-requisitos">Requisitos</a> ·
  <a href="#-como-usar">Como usar</a> ·
  <a href="#-compilar">Compilar</a> ·
  <a href="#-roadmap">Roadmap</a>
</p>

</div>

---

## ✨ O que ele faz

<table>
  <tr>
    <td width="50%" valign="top">
      <h3>🎙️ Reunião ao vivo</h3>
      <p>Grava duas faixas ao mesmo tempo e transcreve enquanto vocês falam:</p>
      <ul>
        <li><b>EU</b>: o seu microfone</li>
        <li><b>OUTROS</b>: o som do computador (Teams, Meet, Zoom…)</li>
      </ul>
      <p>O texto aparece na mini janela em cerca de <b>1,5 s</b> depois de cada fala (p95 abaixo de 3 s).</p>
    </td>
    <td width="50%" valign="top">
      <h3>📁 Arquivo e legenda</h3>
      <p>Importe um áudio ou vídeo e receba a transcrição com o tempo de cada palavra:</p>
      <ul>
        <li>exporta <code>.srt</code>, <code>.txt</code> e <code>.json</code></li>
        <li>editor com os trechos suspeitos destacados</li>
        <li><b>comparar com o Whisper</b> (opcional): mostra lado a lado só os trechos em que os dois modelos discordam, e você escolhe</li>
        <li><b>legenda de vídeo editado</b>: a partir dos brutos transcritos e de um <code>edl.json</code> com os cortes, com verificação no vídeo final</li>
        <li>cerca de <b>15× mais rápido</b> que o tempo real numa CPU comum (1 h de áudio em uns 4 min)</li>
      </ul>
    </td>
  </tr>
  <tr>
    <td valign="top">
      <h3>🪟 Mini janela discreta</h3>
      <ul>
        <li>medidor de sinal por canal e as últimas falas</li>
        <li>⭐ marcar um momento, pausar (libera o microfone) e parar</li>
        <li><b>invisível no compartilhamento de tela</b> (opção 👁)</li>
      </ul>
    </td>
    <td valign="top">
      <h3>🔊 Áudio que aguenta o dia a dia</h3>
      <ul>
        <li>segue a troca de fone, de dispositivo padrão e de perfil do Bluetooth sem parar a gravação</li>
        <li>no alto-falante, reconhece o <b>eco</b> (a voz dos outros entrando pelo seu mic) e o esconde sem apagar</li>
        <li>captura “só o som de um aplicativo” no Windows 11</li>
      </ul>
    </td>
  </tr>
  <tr>
    <td valign="top">
      <h3>🗂️ Sessões</h3>
      <ul>
        <li>transcrição EU/OUTROS com marcas e pausas</li>
        <li>renomear, abrir o áudio, apagar</li>
        <li>o áudio é guardado por 7 dias, 30 dias, sempre ou nunca, e é apagado sozinho ao vencer</li>
      </ul>
    </td>
    <td valign="top">
      <h3>⌨️ Atalhos globais</h3>
      <p>Não conflitam com Teams, Zoom nem Meet e podem ser trocados:</p>
      <table>
        <tr><td><kbd>Ctrl</kbd>+<kbd>Alt</kbd>+<kbd>Shift</kbd>+<kbd>R</kbd></td><td>gravar / parar</td></tr>
        <tr><td><kbd>Ctrl</kbd>+<kbd>Alt</kbd>+<kbd>Shift</kbd>+<kbd>M</kbd></td><td>marcar momento</td></tr>
        <tr><td><kbd>Ctrl</kbd>+<kbd>Alt</kbd>+<kbd>Shift</kbd>+<kbd>O</kbd></td><td>mostrar a mini</td></tr>
      </table>
    </td>
  </tr>
</table>

> [!IMPORTANT]
> **Por que o OuveAI não gera resumo nem ata?**
> Reuniões costumam ter dados pessoais e informações internas da empresa. Por isso, o OuveAI segue os princípios da LGPD (Lei 13.709/2018), sobretudo os de necessidade e segurança: **tudo é processado no seu computador, e nenhuma transcrição ou áudio é enviado a servidores.**
>
> Resumos e atas gerados por IA exigiriam mandar o texto da reunião para um serviço na nuvem, ou rodar um modelo grande demais para um notebook comum. Por isso, ficaram de fora de propósito. Se quiser um resumo, copie a transcrição (em **Sessões › Copiar transcrição**, em Markdown) e leve-a para a ferramenta que a sua empresa autorizar. A decisão de compartilhar fica sempre com você.

---

## 🔒 Privacidade

<table>
  <tr><td>💻</td><td><b>Tudo roda na sua máquina.</b> O reconhecimento de fala usa um modelo local, ajustado para o português do Brasil. Nenhuma transcrição ou áudio é enviado para servidor nenhum.</td></tr>
  <tr><td>🌐</td><td><b>A rede só é usada para baixar componentes</b> (o modelo de fala e, se você quiser, o FFmpeg e o Whisper), uma vez, com o SHA-256 conferido e a partir dos releases do próprio projeto no GitHub (a origem de cada um fica como reserva), <b>e para ver se há versão nova</b>: quando você clica em "Verificar atualização" ou, se ligar o aviso, no máximo uma vez por dia. Dá para baixar os componentes pelo navegador e importar o arquivo.</td></tr>
  <tr><td>📝</td><td><b>Os logs não guardam o que foi dito</b>: nem texto transcrito, nem títulos de reunião, nem nomes de arquivo. Só tempos, contagens e nomes de dispositivo.</td></tr>
  <tr><td>📂</td><td><b>Os dados ficam em <code>%LOCALAPPDATA%</code></b>, fora do perfil móvel da empresa. Apagar uma sessão apaga o texto e o áudio.</td></tr>
</table>

---

## 🖥️ Requisitos

| | Mínimo |
|---|---|
| Sistema | Windows 10 2004 (build 19041) ou mais novo, x64 |
| Memória | 8 GB de RAM (o app usa ~900 MB gravando) |
| CPU | 4 núcleos (~2–3% de CPU numa reunião, num Ryzen 5 5600X) |
| Disco | 2 GB livres (modelo de ~640 MB + áudios, se você guardar). A comparação com o Whisper, opcional, usa mais ~1 GB |
| Permissões | **Não precisa de administrador.** Instala só para o seu usuário. |

O instalador não exige o VC++ Redistributable: o motor de reconhecimento vai linkado no próprio executável.

---

## 🚀 Como usar

1. Baixe o instalador `OuveAI_x.y.z_x64-setup.exe` na [**página da última versão**](https://github.com/freire-andre/OuveAI/releases/latest) e dê dois cliques.
   - Se aparecer "O Windows protegeu o computador", clique em **Mais informações › Executar assim mesmo**. O instalador ainda não tem assinatura digital.
   - Para conferir o arquivo, compare `Get-FileHash .\OuveAI_*_x64-setup.exe` com o `.sha256` publicado junto.
2. No primeiro uso, o assistente:
   - baixa o modelo de fala (ou importa de um arquivo, se a rede da empresa bloquear);
   - testa o microfone e o som do computador;
   - mostra os atalhos.
3. Quando a reunião começar, aperte <kbd>Ctrl</kbd>+<kbd>Alt</kbd>+<kbd>Shift</kbd>+<kbd>R</kbd>. A mini janela abre e o texto vai aparecendo.
4. Aperte de novo para parar. A reunião fica salva em **Sessões**, de onde dá para copiar ou exportar.

Para transcrever ou legendar um áudio ou vídeo, abra **Arquivos** na janela principal e escolha o arquivo (ou arraste para lá). A legenda de um vídeo editado fica em **Legendas**.

Para ser avisado quando sair uma versão nova, ligue a opção em **Configurações › Sobre**. Ela vem desligada.

---

## 🛠️ Compilar

<details>
<summary><b>Pré-requisitos e comandos</b></summary>

<br>

**Ferramentas**
- Rust stable com o toolchain MSVC (`x86_64-pc-windows-msvc`) e o Visual Studio Build Tools (C++)
- Node.js 20 ou mais novo
- WebView2 Runtime (já vem no Windows 11)

**App**

```powershell
npm install
npm run tauri dev      # desenvolvimento
npm run tauri build    # instalador NSIS em target\release\bundle\nsis\
```

Na primeira compilação, o crate `sherpa-onnx` baixa a biblioteca estática (~117 MB). Numa rede restrita, aponte `SHERPA_ONNX_ARCHIVE_DIR` ou `SHERPA_ONNX_LIB_DIR` para uma cópia local.

**CLI** (para testes e uso técnico)

```powershell
cargo run -p ouveai-cli --release -- models download parakeet-tdt-0.6b-v3-ptbr-int8
cargo run -p ouveai-cli --release -- transcribe caminho\do\audio.mp4
cargo run -p ouveai-cli --release -- edl-subs edl.json --check final.mp4   # legenda do vídeo editado + verificação
cargo run -p ouveai-cli --release -- live --help     # replay de dois WAVs no pipeline ao vivo
```

Outros comandos: `paths`, `config`, `ffmpeg`, `capture`.

**Testes**

```powershell
cargo test --workspace
npm run check
```

</details>

<details>
<summary><b>Estrutura do repositório</b></summary>

<br>

| Pasta | Conteúdo |
|---|---|
| `crates/ouveai-core` | Núcleo sem Tauri: ASR, VAD, segmentação ao vivo, eco, legendas, banco SQLite, downloads |
| `crates/ouveai-audio` | Captura WASAPI: microfone, loopback, por aplicativo, troca de dispositivo |
| `crates/ouveai-cli` | Linha de comando sobre o core |
| `src-tauri` | App desktop: janelas, bandeja, atalhos, stealth, comandos |
| `src` | Interface em Svelte 5 |
| `docs` | Plano, ADRs e o resultado de cada fase |
| `image` | Logo, ícone e imagens da marca |

</details>

<details>
<summary><b>Como funciona por dentro</b></summary>

<br>

```
 Microfone ──► WASAPI (16 kHz mono) ──► VAD Silero ──► segmentador ──┐
                                                                     ├──► Parakeet TDT ──► filtro ──► eco ──► SQLite ──► mini / Sessões
 Som do PC ──► WASAPI loopback      ──► VAD Silero ──► segmentador ──┘
```

- **Relógio único:** cada pacote recebe o tempo pelo QPC do Windows, e os buracos viram silêncio, então EU e OUTROS ficam alinhados mesmo quando o loopback para de mandar dados.
- **Cortes:** uma fala fecha depois de 0,8 s de silêncio. Em fala corrida, o corte cai na primeira pausa curta depois de 10 s, nunca no meio de uma palavra.
- **A gravação começa na hora**, enquanto o modelo ainda carrega, e nenhuma fala do começo se perde.
- **WAV por canal**, com o cabeçalho regravado a cada segundo: o arquivo continua válido mesmo se o app cair.

As decisões técnicas estão em [`docs/adr/`](docs/adr) e o plano completo em [`docs/Plano_Revisado.md`](docs/Plano_Revisado.md).

</details>

---

## 🗺️ Roadmap

| Fase | Entrega | Status |
|---|---|---|
| 0 | Spikes técnicos: ASR, captura, empacotamento, download | ✅ |
| 1 | Esqueleto do app, bandeja, atalhos, stealth, instalador | ✅ |
| 2 | Modelos, FFmpeg e assistente de primeiro uso | ✅ |
| 3 | Transcrição de arquivo, legenda `.srt` e editor | ✅ |
| 4 | Captura ao vivo, mini janela e sessões | ✅ |
| 5 | Histórico, busca sem acento, copiar em Markdown, exportar | ✅ |
| 6 | Instalador final, tela de licenças, diagnóstico, piloto | 🧪 piloto (0.9.5) |
| 7 | Comparação com Whisper (opcional) | ✅ |
| 8 | Legenda de vídeo editado (EDL) | ✅ |

---

## 🙏 Créditos

O OuveAI usa estes projetos abertos:

- [NVIDIA Parakeet TDT 0.6B v3](https://huggingface.co/nvidia/parakeet-tdt-0.6b-v3): modelo de fala, **CC-BY-4.0**
- [Ajuste pt-BR do Parakeet](https://huggingface.co/alexandreacff/parakeet-tdt-v3-ptBR-30s) (alexandreacff, treinado com o conjunto [TAGARELA](https://huggingface.co/datasets/freds0/TAGARELA)) e a [conversão para o sherpa-onnx](https://huggingface.co/joaorura/parakeet-tdt-0.6b-v3-ptBR-sherpa-onnx-int8) (joaorura): **CC-BY-4.0**
- [sherpa-onnx](https://github.com/k2-fsa/sherpa-onnx): motor de inferência, Apache-2.0
- [ONNX Runtime](https://github.com/microsoft/onnxruntime): MIT
- [Silero VAD](https://github.com/snakers4/silero-vad): detecção de voz, MIT
- [Whisper large-v3 turbo](https://github.com/openai/whisper) (OpenAI), opcional, usado só na comparação: MIT
- [Tauri](https://tauri.app), [Svelte](https://svelte.dev) e o crate [`wasapi`](https://github.com/HEnquist/wasapi-rs)
- [FFmpeg](https://ffmpeg.org) (baixado à parte, build da gyan.dev, GPL-3.0), usado só para importar arquivos. A cópia e o código-fonte ficam no release [`ffmpeg-9.0.2`](https://github.com/freire-andre/OuveAI/releases/tag/ffmpeg-9.0.2)

---

<div align="center">
<sub>Feito no Brasil 🇧🇷 · Sem licença pública por enquanto, todos os direitos reservados.</sub>
</div>
