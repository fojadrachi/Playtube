; Inno-Setup-Skript fuer den Playtube-Windows-Installer (Playtube-Setup-vX.Y.Z.exe).
;
; Bauen (Inno Setup 6.3+ noetig, https://jrsoftware.org/isinfo.php):
;   ISCC.exe /DAppVersion=2.4.0 packaging\playtube.iss
; Voraussetzung: dist\Playtube\ existiert bereits (PyInstaller-Build, siehe build.ps1).
; Ergebnis: dist\installer\Playtube-Setup-v2.4.0.exe
;
; Wichtig fuer "es soll nie mehrere Versionen geben":
;  - Feste AppId (unten): jede neue Setup-Version erkennt darueber die vorhandene
;    Installation und ueberschreibt sie im SELBEN Ordner (statt eine zweite anzulegen).
;  - Installiert pro Benutzer nach %LOCALAPPDATA%\Programs\Playtube (keine Admin-Rechte
;    noetig, Ordner ist fuer Playtubes eigenen Updater beschreibbar).
;  - Laufende Playtube-Prozesse werden vor dem Kopieren beendet, der alte Programmordner
;    (_internal) wird vor dem Kopieren geleert, damit keine Reste alter Versionen bleiben.
;  - Login/Einstellungen liegen in %APPDATA%\Playtube und bleiben bei Updates erhalten.
;
; Hinweis zur Datei: bewusst reines ASCII (kein BOM), damit die Umlaute-Kodierung von
; Inno Setup nicht ins Spiel kommt.

#ifndef AppVersion
  #define AppVersion "0.0.0"
#endif

#define AppName "Playtube"
#define AppExeName "Playtube.exe"
#define AppHelperName "PlaytubeHelper.exe"

[Setup]
; NIEMALS aendern - identifiziert die Installation ueber alle Versionen hinweg. Muss mit
; _INNO_APP_ID in playtube/updater.py uebereinstimmen.
AppId={{87FBA502-2B84-4C42-A66B-2F03F490912D}
AppName={#AppName}
AppVersion={#AppVersion}
AppVerName={#AppName} {#AppVersion}
AppPublisher=Playtube
AppPublisherURL=https://github.com/fojadrachi/Playtube
AppSupportURL=https://github.com/fojadrachi/Playtube/issues
AppUpdatesURL=https://github.com/fojadrachi/Playtube/releases
VersionInfoVersion={#AppVersion}
DefaultDirName={localappdata}\Programs\{#AppName}
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
; Fensters nur im Tray und reagiert daher nicht zuverlaessig auf die Aufforderung, sich zu
; schliessen, die Inno per CloseApplications senden wuerde.
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

[Files]
Source: "..\dist\Playtube\*"; DestDir: "{app}"; Flags: ignoreversion recursesubdirs createallsubdirs

[Icons]
; {autoprograms} = %APPDATA%\Microsoft\Windows\Start Menu\Programs - derselbe Pfad, den die
; App frueher selbst fuer eine portable Kopie angelegt hat; eine alte Verknuepfung, die
; noch auf einen Downloads-Ordner zeigt, wird dadurch ersetzt.
Name: "{autoprograms}\{#AppName}"; Filename: "{app}\{#AppExeName}"
Name: "{autodesktop}\{#AppName}"; Filename: "{app}\{#AppExeName}"; Tasks: desktopicon

[Registry]
; Dateizuordnung fuer ".play" (Playtube-Patchdateien, siehe playtube/shortcuts.py).
Root: HKCU; Subkey: "Software\Classes\.play"; ValueType: string; ValueName: ""; ValueData: "{#AppName}.PatchFile"; Flags: uninsdeletekey
Root: HKCU; Subkey: "Software\Classes\{#AppName}.PatchFile"; ValueType: string; ValueName: ""; ValueData: "{#AppName}-Patchdatei"; Flags: uninsdeletekey
Root: HKCU; Subkey: "Software\Classes\{#AppName}.PatchFile\DefaultIcon"; ValueType: string; ValueName: ""; ValueData: "{app}\{#AppExeName}"
Root: HKCU; Subkey: "Software\Classes\{#AppName}.PatchFile\shell\open\command"; ValueType: string; ValueName: ""; ValueData: """{app}\{#AppExeName}"" ""%1"""

[Run]
; Normale (sichtbare) Installation: Haekchen "Playtube jetzt starten" am Ende.
Filename: "{app}\{#AppExeName}"; Description: "{#AppName} jetzt starten"; Flags: nowait postinstall skipifsilent
; Stilles Update durch Playtubes eigenen Updater (/RELAUNCH=1): App danach automatisch
; wieder starten.
Filename: "{app}\{#AppExeName}"; Flags: nowait; Check: ShouldRelaunch

[Code]
function ShouldRelaunch(): Boolean;
begin
  Result := ExpandConstant('{param:RELAUNCH|0}') = '1';
end;

procedure StopRunningPlaytube();
var
  ResultCode: Integer;
begin
  { /T beendet auch die Kindprozesse (QtWebEngine); der Hilfsprozess wird zusaetzlich
    einzeln beendet, falls er als Waise ueberlebt hat. }
  Exec(ExpandConstant('{sys}\taskkill.exe'), '/F /T /IM {#AppExeName}', '', SW_HIDE, ewWaitUntilTerminated, ResultCode);
  Exec(ExpandConstant('{sys}\taskkill.exe'), '/F /IM {#AppHelperName}', '', SW_HIDE, ewWaitUntilTerminated, ResultCode);
  { Dateisperren freigeben lassen, bevor kopiert wird. }
  Sleep(800);
end;

function PrepareToInstall(var NeedsRestart: Boolean): String;
begin
  StopRunningPlaytube();
  Result := '';
end;

function InitializeUninstall(): Boolean;
begin
  StopRunningPlaytube();
  Result := True;
end;

procedure CurUninstallStepChanged(CurUninstallStep: TUninstallStep);
begin
  if (CurUninstallStep = usPostUninstall) and (not UninstallSilent()) then
  begin
    if MsgBox('Sollen auch die Einstellungen und der gespeicherte Login von {#AppName} geloescht werden?' + #13#10 +
              '(Ordner: ' + ExpandConstant('{userappdata}\{#AppName}') + ')',
              mbConfirmation, MB_YESNO or MB_DEFBUTTON2) = IDYES then
      DelTree(ExpandConstant('{userappdata}\{#AppName}'), True, True, True);
  end;
end;
