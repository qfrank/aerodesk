; AeroDesk listens on 0.0.0.0:<port> for the phone's LAN WebSocket. Without an
; inbound allow rule Windows Defender Firewall silently drops the phone's TCP
; connect, so the QR pair never works. Add the rule at install time (elevated:
; nsis.installMode = "perMachine") and remove it on uninstall.

!macro NSIS_HOOK_POSTINSTALL
  nsExec::Exec 'netsh advfirewall firewall add rule name=$\"AeroDesk$\" dir=in action=allow program=$\"$INSTDIR\AeroDesk.exe$\" enable=yes profile=any'
!macroend

!macro NSIS_HOOK_POSTUNINSTALL
  nsExec::Exec 'netsh advfirewall firewall delete rule name=$\"AeroDesk$\" program=$\"$INSTDIR\AeroDesk.exe$\"'
!macroend