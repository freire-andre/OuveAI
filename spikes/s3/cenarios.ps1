# Spike S3 · roteiro guiado dos cenários de captura (rodar no PowerShell, na raiz do projeto)
#   .\spikes\s3\cenarios.ps1            -> todos os cenários
#   .\spikes\s3\cenarios.ps1 -So b,d    -> só alguns
# Saída: testdata\s3\<cenário>\ (mic.wav, loop_mm.wav, loop_comm.wav, proc.wav, eventos.csv, niveis.csv, notas.txt)
param([string[]]$So)
# Com -File, '-So a,b' chega como um texto só: separa pelas vírgulas.
$So = @($So | ForEach-Object { $_ -split ',' } | ForEach-Object { $_.Trim() } | Where-Object { $_ })

$ErrorActionPreference = 'Continue'
$root = Split-Path (Split-Path $PSScriptRoot -Parent) -Parent
$exe = Join-Path $root 'target\release\examples\s3_capture.exe'
if (-not (Test-Path $exe)) { throw "Compile antes: cargo build --release -p ouveai-audio --example s3_capture" }

$cenarios = @(
  @{ id = 'c'; titulo = '(c) Silêncio e lacunas do loopback'; secs = 60; proc = $null; passos = @(
      'Pause TODO áudio do PC (YouTube, Spotify etc.).',
      'Depois do Enter: fique 25 s em silêncio total.',
      'Aos ~25 s, dê play num vídeo qualquer por ~15 s e pause de novo.',
      'Aos ~45 s, fale uma frase em voz normal (ex.: "teste do microfone").') }
  @{ id = 'a'; titulo = '(a) Multimídia ≠ comunicação, chamada no Zoom'; secs = 90; proc = $null; passos = @(
      'ANTES do Enter: ligue o EDIFIER W800BT. Em Configurações > Sistema > Som > Mais configurações de som,',
      '   deixe o Sharkoon como "Dispositivo padrão" e o EDIFIER como "Dispositivo de comunicação padrão".',
      'Deixe um vídeo do YouTube tocando baixo (vai para o Sharkoon).',
      'Depois do Enter: abra o Zoom > Configurações > Áudio > "Testar alto-falante" e depois "Testar microfone",',
      '   OU entre em https://zoom.us/test e siga o teste (ele toca um som e repete sua voz).',
      'Fale algumas frases quando o Zoom pedir.') }
  @{ id = 'b'; titulo = '(b) Bluetooth entrando em chamada (troca A2DP → HFP)'; secs = 90; proc = $null; passos = @(
      'ANTES do Enter: deixe o EDIFIER como padrão para TUDO (padrão e comunicação). Toque um vídeo no YouTube (você ouve no EDIFIER).',
      'Depois do Enter: espere ~15 s e então entre no teste do Zoom (https://zoom.us/test) usando o EDIFIER como microfone e alto-falante.',
      'O som do EDIFIER deve ficar "abafado" (perfil de chamada). Fale algumas frases.',
      'Aos ~70 s, saia do Zoom (o som volta ao normal).') }
  @{ id = 'd'; titulo = '(d) Desconectar e reconectar o dispositivo no meio'; secs = 75; proc = $null; passos = @(
      'ANTES do Enter: volte o Sharkoon como padrão para TUDO. Toque um vídeo no YouTube.',
      'Depois do Enter: aos ~15 s, DESCONECTE o cabo USB do Sharkoon.',
      'Aos ~35 s, conecte de novo.',
      'Aos ~55 s, ligue o EDIFIER (ele pode virar o padrão sozinho) e desligue-o aos ~65 s.') }
  @{ id = 'e'; titulo = '(e) Captura só de um aplicativo (Zoom)'; secs = 60; proc = 'Zoom.exe'; passos = @(
      'ANTES do Enter: abra o Zoom e entre em https://zoom.us/test (ou deixe o teste de alto-falante pronto).',
      'Deixe um vídeo do YouTube tocando ao mesmo tempo, no Chrome.',
      'Depois do Enter: rode o teste do Zoom (toque o som e fale). A faixa "proc" deve ter SÓ o Zoom; a loop_mm, os dois.') }
  @{ id = 'f1'; titulo = '(f1) Mic Bluetooth em chamada, tolerância 10 ms'; secs = 45; proc = $null; extra = @('--mic-tol-ms','10'); passos = @(
      'ANTES do Enter: ligue o EDIFIER e deixe-o como padrão para TUDO (o Windows costuma fazer isso sozinho). Nada tocando.',
      'Depois do Enter: espere 3 s e leia em voz normal, devagar, o texto abaixo (pelo microfone do EDIFIER):',
      '   "Hoje vamos revisar o cronograma do projeto com a Verônica e o Guilherme. A entrega da primeira versão fica para sexta-feira,',
      '    e na semana que vem começamos o piloto com cinco colegas. Alguma dúvida sobre o orçamento ou sobre os próximos passos?"',
      'Depois fique em silêncio até acabar.') }
  @{ id = 'f2'; titulo = '(f2) O mesmo, tolerância 200 ms'; secs = 45; proc = $null; extra = @('--mic-tol-ms','200'); passos = @(
      'Mesma preparação do f1 (EDIFIER padrão para tudo, nada tocando).',
      'Depois do Enter: espere 3 s e leia o MESMO texto:',
      '   "Hoje vamos revisar o cronograma do projeto com a Verônica e o Guilherme. A entrega da primeira versão fica para sexta-feira,',
      '    e na semana que vem começamos o piloto com cinco colegas. Alguma dúvida sobre o orçamento ou sobre os próximos passos?"',
      'Depois fique em silêncio até acabar.') }
)

foreach ($c in $cenarios) {
  if ($So -and ($So -notcontains $c.id)) { continue }
  $out = Join-Path $root "testdata\s3\$($c.id)"
  New-Item -ItemType Directory -Force $out | Out-Null
  Write-Host ''
  Write-Host ('=' * 78) -ForegroundColor Cyan
  Write-Host " $($c.titulo)  ·  $($c.secs) s" -ForegroundColor Cyan
  Write-Host ('=' * 78) -ForegroundColor Cyan
  $c.passos | ForEach-Object { Write-Host "  • $_" }
  $r = Read-Host "`nEnter para começar (ou 'p' para pular)"
  if ($r -eq 'p') { continue }
  $args_ = @('--secs', $c.secs, '--out', $out)
  if ($c.proc) { $args_ += @('--proc', $c.proc) }
  if ($c.extra) { $args_ += $c.extra }
  & $exe @args_
  if ($LASTEXITCODE -ne 0) { Write-Host "(o programa terminou com código $LASTEXITCODE — interrompido? Os WAVs valem até o último segundo.)" -ForegroundColor Yellow }
  Write-Host "Cenário '$($c.id)' gravado." -ForegroundColor Green
  $nota = Read-Host "`nAlgo estranho? Descreva em 1 linha (Enter para pular)"
  "$(Get-Date -Format s) · $nota" | Out-File -Append -Encoding utf8 (Join-Path $out 'notas.txt')
}
Write-Host "`nPronto! Os resultados estão em testdata\s3\. Avise o Claude para analisar." -ForegroundColor Green
