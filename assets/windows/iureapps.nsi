; Instalador -setup.exe de Iurefficient Apps, sólo para el usuario actual (sin administrador):
; se instala en %LOCALAPPDATA%\Programs\Iurefficient Apps, así el lanzador puede actualizarse
; solo sin pedir permisos. Se compila en CI:
;   makensis /DVERSION=0.1.9 /DEXE=ruta\iureapps.exe /DOUTFILE=iureapps-windows-x64-setup.exe assets\windows\iureapps.nsi

Unicode true
!include "MUI2.nsh"

!ifndef VERSION
  !error "falta /DVERSION=x.y.z"
!endif
!ifndef EXE
  !error "falta /DEXE=ruta del iureapps.exe"
!endif
!ifndef OUTFILE
  !define OUTFILE "iureapps-windows-x64-setup.exe"
!endif

!define NAME "Iurefficient Apps"
!define UNINST_KEY "Software\Microsoft\Windows\CurrentVersion\Uninstall\iureapps"

Name "${NAME}"
OutFile "${OUTFILE}"
InstallDir "$LOCALAPPDATA\Programs\${NAME}"
InstallDirRegKey HKCU "${UNINST_KEY}" "InstallLocation"
RequestExecutionLevel user
SetCompressor /SOLID lzma
BrandingText "${NAME} ${VERSION}"

VIProductVersion "${VERSION}.0"
VIAddVersionKey "ProductName" "${NAME}"
VIAddVersionKey "CompanyName" "Iurefficient"
VIAddVersionKey "FileDescription" "Instalador de ${NAME}"
VIAddVersionKey "FileVersion" "${VERSION}"
VIAddVersionKey "ProductVersion" "${VERSION}"
VIAddVersionKey "LegalCopyright" "Iurefficient"

!define MUI_ICON "iureapps.ico"
!define MUI_UNICON "iureapps.ico"
!define MUI_ABORTWARNING
!define MUI_FINISHPAGE_RUN "$INSTDIR\iureapps.exe"
!define MUI_FINISHPAGE_RUN_TEXT "Abrir ${NAME}"

!insertmacro MUI_PAGE_DIRECTORY
!insertmacro MUI_PAGE_INSTFILES
!insertmacro MUI_PAGE_FINISH
!insertmacro MUI_UNPAGE_CONFIRM
!insertmacro MUI_UNPAGE_INSTFILES
!insertmacro MUI_LANGUAGE "Spanish"

Section "Instalar"
  SetOutPath "$INSTDIR"
  ; Si el lanzador está abierto, se cierra para poder sustituirlo.
  nsExec::Exec 'taskkill /IM iureapps.exe /F'
  Sleep 500
  File "/oname=iureapps.exe" "${EXE}"
  WriteUninstaller "$INSTDIR\desinstalar.exe"
  CreateShortcut "$SMPROGRAMS\${NAME}.lnk" "$INSTDIR\iureapps.exe" "" "$INSTDIR\iureapps.exe" 0

  WriteRegStr HKCU "${UNINST_KEY}" "DisplayName" "${NAME}"
  WriteRegStr HKCU "${UNINST_KEY}" "DisplayVersion" "${VERSION}"
  WriteRegStr HKCU "${UNINST_KEY}" "Publisher" "Iurefficient"
  WriteRegStr HKCU "${UNINST_KEY}" "DisplayIcon" "$INSTDIR\iureapps.exe"
  WriteRegStr HKCU "${UNINST_KEY}" "InstallLocation" "$INSTDIR"
  WriteRegStr HKCU "${UNINST_KEY}" "URLInfoAbout" "https://github.com/ellaguno/iureapps"
  WriteRegStr HKCU "${UNINST_KEY}" "UninstallString" '"$INSTDIR\desinstalar.exe"'
  WriteRegStr HKCU "${UNINST_KEY}" "QuietUninstallString" '"$INSTDIR\desinstalar.exe" /S'
  WriteRegDWORD HKCU "${UNINST_KEY}" "NoModify" 1
  WriteRegDWORD HKCU "${UNINST_KEY}" "NoRepair" 1
SectionEnd

Section "Uninstall"
  nsExec::Exec 'taskkill /IM iureapps.exe /F'
  Sleep 500
  Delete "$SMPROGRAMS\${NAME}.lnk"
  Delete "$INSTDIR\iureapps.exe"
  Delete "$INSTDIR\desinstalar.exe"
  RMDir "$INSTDIR"
  DeleteRegKey HKCU "${UNINST_KEY}"
SectionEnd
