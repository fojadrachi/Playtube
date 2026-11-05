; Setup für Playtube (Inno Setup 6). Bauen über build-release.ps1 im Hauptordner, das setzt
; AppVersion und den Pfad zur .exe.
;
; Feste AppId (dieselbe wie bei allen früheren Playtube-Versionen): jedes Setup erkennt eine
; vorhandene Installation - auch die alte Python-/Qt-Version - und ersetzt sie im selben Ordner.
; Muss mit der Updater-Logik in src/updater.rs zusammenpassen (Setup-Name "Playtube-Setup-<v>.exe").

#ifndef AppVersion
  #define AppVersion "0.0.0"
#endif
#ifndef SourceExe
  #define SourceExe "..\target\release\playtube.exe"
#endif
#define AppName "Playtube"
#define AppExeName "Playtube.exe"

[Setup]
; NIEMALS ändern - identifiziert die Playtube-Installation über alle Versionen hinweg.
AppId={{87FBA502-2B84-4C42-A66B-2F03F490912D}
AppName={#AppName}
AppVersion={#AppVersion}
AppVerName={#AppName} {#AppVersion}
AppPublisher=Fojadrachi
AppPublisherURL=https://github.com/fojadrachi/Playtube
AppSupportURL=https://github.com/fojadrachi/Playtube/issues
AppUpdatesURL=https://github.com/fojadrachi/Playtube/releases
; Installation pro Nutzer, keine Adminrechte nötig
PrivilegesRequired=lowest
DefaultDirName={localappdata}\Programs\{#AppName}
DisableDirPage=auto
DisableProgramGroupPage=yes
; Willkommensseite mit dem großen Bild zeigen (bei stillen Updates wird sie übersprungen).
DisableWelcomePage=no
UsePreviousAppDir=yes
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
MinVersion=10.0
OutputDir=..\dist
OutputBaseFilename=Playtube-Setup-{#AppVersion}
SetupIconFile=..\assets\icon.ico
UninstallDisplayIcon={app}\{#AppExeName}
UninstallDisplayName={#AppName}
WizardStyle=modern
; Eigene Bilder (installer\make-images.ps1) für 100 bis 200 % Anzeigeskalierung; Inno wählt das
; passende und skaliert es auf die Fläche.
WizardImageFile=wizard-100.bmp,wizard-125.bmp,wizard-150.bmp,wizard-175.bmp,wizard-200.bmp
WizardSmallImageFile=small-100.bmp,small-125.bmp,small-150.bmp,small-175.bmp,small-200.bmp
WizardImageStretch=yes
Compression=lzma2/max
SolidCompression=yes
; Prozesse beenden wir selbst (siehe [Code]): Playtube versteckt sich beim Schließen nur im Tray.
CloseApplications=force
RestartApplications=no
VersionInfoVersion={#AppVersion}
VersionInfoProductName={#AppName}

[Languages]
Name: "de"; MessagesFile: "compiler:Languages\German.isl"
Name: "en"; MessagesFile: "compiler:Default.isl"

[Tasks]
Name: "desktopicon"; Description: "{cm:CreateDesktopIcon}"; GroupDescription: "{cm:AdditionalIcons}"; Flags: unchecked

[InstallDelete]
; Reste der früheren Python-Versionen (PyInstaller-Laufzeit, Qt-Hilfsprozess).
Type: filesandordirs; Name: "{app}\_internal"
Type: files; Name: "{app}\PlaytubeHelper.exe"

[Files]
Source: "{#SourceExe}"; DestDir: "{app}"; DestName: "{#AppExeName}"; Flags: ignoreversion

[Icons]
Name: "{autoprograms}\{#AppName}"; Filename: "{app}\{#AppExeName}"
Name: "{autodesktop}\{#AppName}"; Filename: "{app}\{#AppExeName}"; Tasks: desktopicon

[Run]
; Auch nach einem stillen Update (aus der App heraus) wieder starten - daher kein skipifsilent.
Filename: "{app}\{#AppExeName}"; Description: "{cm:LaunchProgram,{#AppName}}"; Flags: nowait postinstall

[Code]
function PowerShellPath(): String;
begin
  Result := ExpandConstant('{sys}\WindowsPowerShell\v1.0\powershell.exe');
end;

procedure WaitForOldInstance();
var
  OldPid, ResultCode: Integer;
begin
  { Der Updater der alten Python-Version übergibt seine Prozess-ID (/WAITPID=...). }
  OldPid := StrToIntDef(ExpandConstant('{param:WAITPID|0}'), 0);
  if OldPid > 0 then
    Exec(PowerShellPath(),
         '-NoProfile -NonInteractive -WindowStyle Hidden -Command "Wait-Process -Id ' + IntToStr(OldPid) + ' -Timeout 20 -ErrorAction SilentlyContinue"',
         '', SW_HIDE, ewWaitUntilTerminated, ResultCode);
end;

procedure StopRunningPlaytube();
var
  ResultCode: Integer;
begin
  { Bewusst ohne "/T": das Setup kann ein Kindprozess von Playtube sein (Update aus der App). }
  Exec(ExpandConstant('{sys}\taskkill.exe'), '/F /IM {#AppExeName}', '', SW_HIDE, ewWaitUntilTerminated, ResultCode);
  Exec(ExpandConstant('{sys}\taskkill.exe'), '/F /IM PlaytubeHelper.exe', '', SW_HIDE, ewWaitUntilTerminated, ResultCode);
  Sleep(800);
end;

function PrepareToInstall(var NeedsRestart: Boolean): String;
begin
  WaitForOldInstance();
  StopRunningPlaytube();
  Result := '';
end;

function InitializeUninstall(): Boolean;
begin
  StopRunningPlaytube();
  Result := True;
end;

procedure CurUninstallStepChanged(CurUninstallStep: TUninstallStep);
var
  DataDir: String;
begin
  if (CurUninstallStep = usPostUninstall) and (not UninstallSilent()) then
  begin
    DataDir := ExpandConstant('{userappdata}\{#AppName}');
    if DirExists(DataDir) then
      if MsgBox('Sollen auch die Einstellungen und der gespeicherte Login von {#AppName} gelöscht werden?'#13#10#13#10 + DataDir,
                mbConfirmation, MB_YESNO or MB_DEFBUTTON2) = IDYES then
        DelTree(DataDir, True, True, True);
  end;
end;
