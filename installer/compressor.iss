; Inno Setup script - built by CI:  ISCC.exe /DMyAppVersion=0.2.0 installer\compressor.iss
#ifndef MyAppVersion
  #define MyAppVersion "0.2.0"
#endif
#define MyAppName "Video Compressor"
#define MyAppExe "compressor.exe"

[Setup]
AppId={{8F3C2B6E-5A1D-4C7B-9E42-1D6A7B0C9F31}
AppName={#MyAppName}
AppVersion={#MyAppVersion}
AppPublisher=itsabubakarshf
AppPublisherURL=https://github.com/itsabubakarshf/compressor
AppSupportURL=https://github.com/itsabubakarshf/compressor
DefaultDirName={autopf}\{#MyAppName}
DefaultGroupName={#MyAppName}
DisableProgramGroupPage=yes
; Installs just for you, no administrator rights needed (the wizard can switch to "all users").
PrivilegesRequired=lowest
PrivilegesRequiredOverridesAllowed=dialog
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
OutputDir=..\dist
OutputBaseFilename=VideoCompressor-Setup-windows
SetupIconFile=..\assets\icon.ico
UninstallDisplayIcon={app}\{#MyAppExe}
InfoBeforeFile=before-install.txt
Compression=lzma2
SolidCompression=yes
WizardStyle=modern

[Languages]
Name: "english"; MessagesFile: "compiler:Default.isl"

[Tasks]
Name: "getffmpeg"; Description: "Download ffmpeg now (needed to process videos; about 100-200 MB, one time only)"; GroupDescription: "Video engine (ffmpeg):"; Check: NeedFfmpeg
Name: "desktopicon"; Description: "Create a &desktop shortcut"; GroupDescription: "Shortcuts:"; Flags: unchecked

[Files]
Source: "..\dist\VideoCompressor-portable-windows.exe"; DestDir: "{app}"; DestName: "{#MyAppExe}"; Flags: ignoreversion

[Icons]
Name: "{group}\{#MyAppName}"; Filename: "{app}\{#MyAppExe}"
Name: "{group}\Set up ffmpeg (download)"; Filename: "{app}\{#MyAppExe}"; Parameters: "--setup-ffmpeg"; Comment: "Download the ffmpeg video engine"
Name: "{group}\Uninstall {#MyAppName}"; Filename: "{uninstallexe}"
Name: "{autodesktop}\{#MyAppName}"; Filename: "{app}\{#MyAppExe}"; Tasks: desktopicon

[Run]
Filename: "{app}\{#MyAppExe}"; Parameters: "--setup-ffmpeg"; StatusMsg: "Downloading ffmpeg (this can take a few minutes)..."; Tasks: getffmpeg; Flags: waituntilterminated
Filename: "{app}\{#MyAppExe}"; Description: "Launch {#MyAppName}"; Flags: nowait postinstall skipifsilent

[UninstallDelete]
; ffmpeg downloaded by the app lives here
Type: filesandordirs; Name: "{localappdata}\compressor"

[Code]
function NeedFfmpeg(): Boolean;
begin
  Result := not (FileExists(ExpandConstant('{localappdata}\compressor\ffmpeg\ffmpeg.exe'))
              or (FileSearch('ffmpeg.exe', GetEnv('PATH')) <> ''));
end;
