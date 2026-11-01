# Spike S4 · Download do modelo (retomada, hash, proxy, espelho)

> 31/10/2026 · `crates/ouveai-core/examples/s4_download.rs` (ureq 3.5, rustls + verificador do Windows + proxy do Windows) · diagnóstico `spikes/s4/rede-corporativa.ps1`
> Rede doméstica, sem proxy, com o Kaspersky pessoal ativo. **O notebook corporativo ainda não foi testado** (o usuário trabalha em casa, veja "Pendente").

## Resultados nesta máquina

| Teste | Resultado |
|---|---|
| Silero completo (GitHub) | ✅ 643854 bytes; SHA-256 bate com o catálogo |
| Parakeet com queda simulada (GitHub) | ✅ 1ª rodada: 200, parou em 20 MB. 2ª: **206** com `Content-Range: bytes 20971520-…` e `If-Range` pelo ETag. Os 41,9 MB do `.part` são idênticos, byte a byte, ao arquivo baixado no S1 |
| Redirecionamento GitHub | `github.com → release-assets.githubusercontent.com` (URL assinada, válida por ~1 h). O `ureq` repete o `Range` e o `If-Range` depois do redirecionamento |
| Espelho Hugging Face | ✅ Mesmos arquivos (SHA-256 iguais aos extraídos do release). `huggingface.co → us.aws.cdn.hf.co`; retomada com 206, bytes idênticos |
| Velocidade (amostras curtas) | GitHub 31–42 MB/s · HF 1,3–1,6 MB/s (≈ 7 min para o encoder). O GitHub fica como fonte principal |
| Inspeção de HTTPS | O Kaspersky pessoal assina o HTTPS **do PowerShell**, mas não interceptou o nosso exe: até as raízes embutidas (`--webpki`) funcionaram. Este teste **não** prova o caso corporativo |

## Decisões (entram na Fase 2, `core::models`)

1. **Cliente HTTP: `ureq` 3** (síncrono e leve; a thread de download é só uma).
   - Features: `rustls` + **`platform-verifier`**. Valida o certificado pelo repositório do Windows, onde o TI instala a raiz do proxy que inspeciona o HTTPS. A lista embutida falharia nesse caso.
   - **`win-system-proxy`**: lê as variáveis de proxy e o `ProxyServer` do Windows.
2. **Retomada:** `<dest>.part` + `<dest>.part.etag`.
   - Na retomada manda `Range: bytes=N-` + `If-Range: <etag>`.
   - Aceita 206 só se o `Content-Range` começar em N. Com 200, recomeça do zero.
   - O hash é incremental: relê o `.part` e segue com o fluxo.
3. **Verificação:** tamanho + SHA-256 (BCrypt do Windows, sem crate extra) contra o catálogo **antes** de renomear. Arquivo divergente nunca é usado.
4. **Fontes:** GitHub releases (principal) → Hugging Face fixado por commit (espelho). Para baixar do HF, os arquivos soltos são melhores que o `.tar.bz2`.
5. **Logs:** só host + caminho. A query das URLs assinadas é credencial temporária.

## Riscos abertos

- **PAC/WPAD:** o `ureq` não lê `AutoConfigURL` nem a autodetecção. Se a empresa usa PAC, o app terá de resolver o proxy com a WinHTTP (`WinHttpGetProxyForUrl`) e passá-lo ao `ureq`, o que dá pouco código.
- **Proxy com autenticação integrada (NTLM/Kerberos):** o `ureq` não suporta. Se aparecer, o plano B é baixar pela própria WinHTTP, que autentica com o login do Windows. Também pouco código, e sem dependência nova.
- Bloqueio de domínio (`*.githubusercontent.com`, `*.hf.co`) por categoria → o plano já prevê importar o modelo de um arquivo ou de uma pasta da rede (Fase 2).

## Notebook corporativo em casa (31/10/2026, rodado pelo usuário)

| | VPN desligada | VPN ligada (GlobalProtect, só para o SAP) |
|---|---|---|
| Proxy (WinHTTP, registro, PAC, variáveis) | nenhum | nenhum |
| Emissor do HTTPS | público: Sectigo (github.com), Let's Encrypt (release-assets), Amazon (HF) → **sem inspeção** | igual |
| GET com `Range` (1 MB) | GitHub 206 (1,4 MB/s) · HF 206 (0,5 MB/s) | GitHub 206 (3,2 MB/s) · HF 206 (0,6 MB/s) |
| Agentes | Defender (`MpDefenderCoreService`, `DefenderAiPlatformHost`), GlobalProtect (`PanGPS`/`PanGPA`), Dell SupportAssist | + `PanGpHipMp` |

**Conclusão:** neste ambiente, nenhum dos riscos abertos (PAC, autenticação, bloqueio de domínio) aparece. A VPN é dividida (split tunnel) e não passa pelo GitHub/HF. O S4 está fechado. O ponto de atenção passa a ser o **Defender** com o exe e o instalador (SmartScreen e reputação de exe não assinado), na Fase 6.

### Roteiro usado

O usuário trabalha de casa. O caso real dos colegas é **o notebook corporativo na rede doméstica**, possivelmente com VPN e/ou agente de proxy na nuvem. Sem rodar nenhum exe nosso (D4: falar com o TI antes), colar `spikes/s4/rede-corporativa.ps1` no Windows PowerShell, **com a VPN desligada e ligada**. Ele mostra:
- o proxy (WinHTTP, registro, PAC, variáveis);
- os agentes de segurança;
- o emissor do certificado de cada host;
- um GET com `Range` de 1 MB no GitHub e no HF, via `curl.exe`.

Leitura do resultado:
- **Emissor ≠ DigiCert/Sectigo/Let's Encrypt etc.:** há inspeção de HTTPS → confirma o `platform-verifier`.
- **`AutoConfigURL` preenchido:** implementar o PAC via WinHTTP.
- **407 no curl:** proxy com autenticação → plano B WinHTTP.
- **Erro de conexão ou 403 em um dos hosts:** domínio bloqueado → espelho ou importação manual.

O teste do exe real no notebook corporativo fica para o piloto (Fase 6), com o instalador assinado e o OK do TI.
