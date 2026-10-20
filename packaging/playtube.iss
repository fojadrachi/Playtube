; Inno-Setup-Skript fuer den Playtube-Windows-Installer (Playtube-Setup-vX.Y.Z.exe).
;
; Bauen (Inno Setup 6.3+ noetig, https://jrsoftware.org/isinfo.php):
;   ISCC.exe /DAppVersion=1.0.0 packaging\playtube.iss
; Voraussetzung: dist\Playtube\ existiert bereits (PyInstaller-Build, siehe build.ps1).
; Ergebnis: dist\installer\Playtube-Setup-v1.0.0.exe
;
;  - Feste AppId (dieselbe wie bei der frueheren Qt-Version bis 4.5.1): jede Setup-Version
;    erkennt darueber die vorhandene Installation - auch eine alte Qt-Playtube - und
;    ueberschreibt sie im SELBEN Ordner (%LOCALAPPDATA%\Programs\Playtube). Muss mit
;    _INNO_APP_ID in playtube/updater.py uebereinstimmen.
;  - Reste der Qt-Version (PlaytubeHelper.exe, alte _internal-Laufzeit) werden dabei entfernt.
;
; Hinweis zur Datei: bewusst reines ASCII (kein BOM), damit die Umlaute-Kodierung von
; Inno Setup nicht ins Spiel kommt.

#ifndef AppVersion
  #define AppVersion "0.0.0"
#endif

#define AppName "Playtube"
#define AppDirName "Playtube"
#define AppExeName "Playtube.exe"
; Hilfsprozess der frueheren Qt-Version - wird beim Update beendet und entfernt.
#define OldHelperName "PlaytubeHelper.exe"

[Setup]
; NIEMALS aendern - identifiziert die Playtube-Installation ueber alle Versionen hinweg.
AppId={{87FBA502-2B84-4C42-A66B-2F03F490912D}
AppName={#AppName}
AppVersion={#AppVersion}
AppVerName={#AppName} {#AppVersion}
AppPublisher=Playtube
AppPublisherURL=https://github.com/fojadrachi/Playtube
AppSupportURL=https://github.com/fojadrachi/Playtube/issues
AppUpdatesURL=https://github.com/fojadrachi/Playtube/releases
VersionInfoVersion={#AppVersion}
DefaultDirName={localappdata}\Programs\{#AppDirName}
DisableProgramGroupPage=yes
PrivilegesRequired=lowest
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
OutputDir=..\dist\installer
OutputBaseFilename=Playtube-Setup-v{#AppVersion}
SetupIconFile=..\assets\icon.ico
UninstallDisplayIcon={app}\{#AppExeName}
UninstallDisplayName={#AppName}
Compression=lzma2/max
SolidCompression=yes
WizardStyle=modern
; Prozesse beenden wir selbst (siehe [Code]) - Playtube versteckt sich beim Schliessen des
; Fensters nur im Tray und reagiert daher nicht zuverlaessig auf CloseApplications.
CloseApplications=no
RestartApplications=no

[Languages]
Name: "german"; MessagesFile: "compiler:Languages\German.isl"
Name: "english"; MessagesFile: "compiler:Default.isl"

[Tasks]
Name: "desktopicon"; Description: "{cm:CreateDesktopIcon}"; GroupDescription: "{cm:AdditionalIcons}"; Flags: unchecked

[InstallDelete]
; Vor dem Kopieren die PyInstaller-Laufzeit der alten Version entfernen, damit nach einem
; Update keine Dateien einer aelteren Version uebrig bleiben.
Type: filesandordirs; Name: "{app}\_internal"
Type: files; Name: "{app}\{#OldHelperName}"

[Files]
Source: "..\dist\Playtube\*"; DestDir: "{app}"; Flags: ignoreversion recursesubdirs createallsubdirs

[Icons]
Name: "{autoprograms}\{#AppName}"; Filename: "{app}\{#AppExeName}"
Name: "{autodesktop}\{#AppName}"; Filename: "{app}\{#AppExeName}"; Tasks: desktopicon

[Run]
; Normale (sichtbare) Installation: Haekchen "Playtube jetzt starten" am Ende.
Filename: "{app}\{#AppExeName}"; Description: "{#AppName} jetzt starten"; Flags: nowait postinstall skipifsilent
; Stilles Update durch den eigenen Updater (/RELAUNCH=1): App danach automatisch wieder starten.
Filename: "{app}\{#AppExeName}"; Flags: nowait; Check: ShouldRelaunch

[Code]
function ShouldRelaunch(): Boolean;
begin
  Result := ExpandConstant('{param:RELAUNCH|0}') = '1';
end;

function PowerShellPath(): String;
begin
  Result := ExpandConstant('{sys}\WindowsPowerShell\v1.0\powershell.exe');
end;

procedure WaitForOldInstance();
var
  OldPid, ResultCode: Integer;
begin
  { Der Updater startet Setup aus der laufenden App heraus und uebergibt deren Prozess-ID
    (/WAITPID=...). Setup wartet, bis sich diese Instanz selbst beendet hat (max. 20 s). }
  OldPid := StrToIntDef(ExpandConstant('{param:WAITPID|0}'), 0);
  if OldPid > 0 then
    Exec(PowerShellPath(),
         '-NoProfile -NonInteractive -WindowStyle Hidden -Command "Wait-Process -Id ' + IntToStr(OldPid) + ' -Timeout 20 -ErrorAction SilentlyContinue"',
         '', SW_HIDE, ewWaitUntilTerminated, ResultCode);
end;

procedure StopLeftoverProcesses();
var
  ResultCode: Integer;
begin
  { WICHTIG: bewusst OHNE "/T" - der Updater startet Setup aus Playtube.exe heraus, Setup ist
    also ein Kindprozess; "/T" wuerde den Installer selbst mit abschiessen. }
  Exec(ExpandConstant('{sys}\taskkill.exe'), '/F /IM {#AppExeName}', '', SW_HIDE, ewWaitUntilTerminated, ResultCode);
  Exec(ExpandConstant('{sys}\taskkill.exe'), '/F /IM {#OldHelperName}', '', SW_HIDE, ewWaitUntilTerminated, ResultCode);

  { WebView2-Kindprozesse (msedgewebview2.exe), die noch aus dem Installationsordner laufen,
    wuerden Dateien sperren - gezielt nach Pfad beenden. }
  if DirExists(ExpandConstant('{app}')) then
    Exec(PowerShellPath(),
         '-NoProfile -NonInteractive -WindowStyle Hidden -Command "$d = ''' + ExpandConstant('{app}') + '\''; ' +
         'Get-CimInstance Win32_Process | Where-Object { $_.ExecutablePath -and $_.ExecutablePath.StartsWith($d, [StringComparison]::OrdinalIgnoreCase) } | ' +
         'ForEach-Object { Stop-Process -Id $_.ProcessId -Force -ErrorAction SilentlyContinue }"',
         '', SW_HIDE, ewWaitUntilTerminated, ResultCode);

  { Dateisperren freigeben lassen, bevor kopiert wird. }
  Sleep(800);
end;

function PrepareToInstall(var NeedsRestart: Boolean): String;
begin
  WaitForOldInstance();
  StopLeftoverProcesses();
  Result := '';
end;

function InitializeUninstall(): Boolean;
begin
  StopLeftoverProcesses();
  Result := True;
end;

procedure CurUninstallStepChanged(CurUninstallStep: TUninstallStep);
begin
  if (CurUninstallStep = usPostUninstall) and (not UninstallSilent()) then
  begin
    if MsgBox('Sollen auch die Einstellungen und der gespeicherte Login von {#AppName} geloescht werden?' + #13#10 +
              '(Ordner: ' + ExpandConstant('{userappdata}\{#AppDirName}') + ')',
              mbConfirmation, MB_YESNO or MB_DEFBUTTON2) = IDYES then
      DelTree(ExpandConstant('{userappdata}\{#AppDirName}'), True, True, True);
  end;
end;
