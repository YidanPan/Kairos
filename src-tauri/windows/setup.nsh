Var KairosHasData

!macro NSIS_HOOK_PREINSTALL
  StrCpy $KairosHasData "0"
  ${If} ${FileExists} "$APPDATA\${BUNDLEID}\kairos.sqlite3"
    StrCpy $KairosHasData "1"
    InitPluginsDir
    File /oname=$PLUGINSDIR\kairos-backup.exe "${MAINBINARYSRCPATH}"
    ClearErrors
    ExecWait '"$PLUGINSDIR\kairos-backup.exe" --backup-before-update' $0
    ${If} ${Errors}
    ${OrIf} $0 <> 0
      MessageBox MB_OK|MB_ICONSTOP "Kairos could not back up the existing data. Check the backup folder and try again." /SD IDOK
      SetErrorLevel 2
      Abort
    ${EndIf}
    Delete "$PLUGINSDIR\kairos-backup.exe"
  ${EndIf}
!macroend

!macro NSIS_HOOK_POSTINSTALL
  ${If} $KairosHasData = "0"
    WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Run" "${BUNDLEID}" '$\"$INSTDIR\${MAINBINARYNAME}.exe$\"'
  ${EndIf}
!macroend

!macro NSIS_HOOK_POSTUNINSTALL
  ${If} $UpdateMode <> 1
    DeleteRegValue HKCU "Software\Microsoft\Windows\CurrentVersion\Run" "${BUNDLEID}"
  ${EndIf}
!macroend
