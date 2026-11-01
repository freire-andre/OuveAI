; Ganchos do instalador NSIS do OuveAI (bundle.windows.nsis.installerHooks).

; Requisito mínimo (D6): Windows 10 2004 (build 19041). Antes disso faltam a captura
; de áudio por processo e o "invisível no compartilhamento".
!macro NSIS_HOOK_PREINSTALL
  ReadRegStr $0 HKLM "SOFTWARE\Microsoft\Windows NT\CurrentVersion" "CurrentBuildNumber"
  ${If} $0 != ""
  ${AndIf} $0 < 19041
    MessageBox MB_ICONSTOP "O OuveAI precisa do Windows 10 versão 2004 ou mais novo.$\nEste computador está no build $0. Atualize o Windows e tente de novo." /SD IDOK
    SetErrorLevel 3
    Quit
  ${EndIf}
!macroend
