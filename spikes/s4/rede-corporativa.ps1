# Spike S4 · diagnóstico de rede no notebook corporativo (só ferramentas do Windows: PowerShell e curl.exe).
# Não instala nada e baixa ~2 MB. Abra o "Windows PowerShell", COLE este bloco inteiro e mande a saída.
# Rode duas vezes: com a VPN desligada e com a VPN ligada.
$hosts = 'github.com', 'release-assets.githubusercontent.com', 'huggingface.co', 'us.aws.cdn.hf.co'
"== proxy"
netsh winhttp show proxy | Select-String 'Direct|Proxy|Bypass'
$is = Get-ItemProperty 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Internet Settings'
"ProxyEnable=$($is.ProxyEnable) ProxyServer=$($is.ProxyServer) AutoConfigURL=$($is.AutoConfigURL) AutoDetect=$($is.AutoDetect)"
$px = [Net.WebRequest]::GetSystemWebProxy().GetProxy([uri]'https://github.com').AbsoluteUri
"proxy p/ github: $px"
"HTTPS_PROXY=$env:HTTPS_PROXY HTTP_PROXY=$env:HTTP_PROXY"
"== agentes de rede/segurança"
(Get-Process | Where-Object { $_.Name -match 'zscaler|zsatunnel|netskope|stagent|forcepoint|umbrella|pangp|vpnagent|forticlient|cisco|sase|crowdstrike|csfalcon|sentinel|defender|kaspersky|avp' } | Select-Object -ExpandProperty Name -Unique) -join ', '
"== quem assina o HTTPS (emissor do certificado)"
foreach ($h in $hosts) {
  try {
    $t = New-Object Net.Sockets.TcpClient($h, 443)
    $s = New-Object Net.Security.SslStream($t.GetStream(), $false, { $true })
    $s.AuthenticateAsClient($h)
    "$h : $($s.RemoteCertificate.Issuer)"
    $t.Close()
  } catch { "$h : ERRO $($_.Exception.Message)" }
}
"== download com Range (1 MB do meio de cada arquivo)"
$x = @(); if ($px -notlike 'https://github.com*') { $x = @('-x', $px) }
$urls = 'https://github.com/k2-fsa/sherpa-onnx/releases/download/asr-models/sherpa-onnx-nemo-parakeet-tdt-0.6b-v3-int8.tar.bz2',
        'https://huggingface.co/csukuangfj/sherpa-onnx-nemo-parakeet-tdt-0.6b-v3-int8/resolve/2bda32ec70b097a55adaa07d9a7173915b43cc78/encoder.int8.onnx'
foreach ($u in $urls) {
  curl.exe -sS -L @x -r 1048576-2097151 -o NUL -w "%{http_code} %{size_download}B %{speed_download}B/s %{time_total}s via %{url_effective}`n" $u 2>&1 |
    ForEach-Object { "$_" -replace '\?.*', '' }
}
"== fim"
