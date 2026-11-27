; Lotus Download Manager - Inno Setup 6 script.
; Built by .github/workflows/release-desktop.yml, one installer per arch:
;   ISCC.exe /DAppVer=0.1.2 /DMyArch=x64  /DOutName=x64      lotus-dm.iss
;   ISCC.exe /DAppVer=0.1.2 /DMyArch=x86  /DOutName=x86      lotus-dm.iss
;   ISCC.exe /DAppVer=0.1.2 /DMyArch=both /DOutName=universal lotus-dm.iss
; The installer is a normal GUI wizard: it never opens a console window.

#ifndef AppVer
  #define AppVer "0.0.0"
#endif
#ifndef MyArch
  #define MyArch "x64"
#endif
#ifndef OutName
  #define OutName "x64"
#endif

[Setup]
; Same AppId for every build, so a new installer upgrades (or repairs) an
; existing install in place instead of stacking a second copy in Apps.
AppId={{6F0C1B52-2C8E-4B7A-9E31-5AB7D2C4E9D1}
AppName=Lotus Download Manager
AppVersion={#AppVer}
AppVerName=Lotus Download Manager {#AppVer}
AppPublisher=b0dr7
AppPublisherURL=https://github.com/b0dr7/lotus-dm
AppSupportURL=https://github.com/b0dr7/lotus-dm/issues
AppUpdatesURL=https://github.com/b0dr7/lotus-dm/releases
DefaultDirName={autopf}\Lotus Download Manager
DefaultGroupName=Lotus Download Manager
DisableProgramGroupPage=yes
UninstallDisplayName=Lotus Download Manager
UninstallDisplayIcon={app}\LotusDM.exe
SetupIconFile=..\assets\lotus.ico
OutputDir=..\dist
OutputBaseFilename=LotusDM-windows-{#OutName}
Compression=lzma2/max
SolidCompression=yes
WizardStyle=modern
; Per-user install by default (no UAC); the user can still choose "for all
; users" in the dialog, which asks for elevation.
PrivilegesRequired=lowest
PrivilegesRequiredOverridesAllowed=dialog
; Repairs/upgrades reuse the same install; no parallel copies.

; A 64-bit installer must not hand a 64-bit exe to a 32-bit system.
#if MyArch == "x64"
ArchitecturesAllowed=x64
ArchitecturesInstallIn64BitMode=x64
#endif

[Languages]
Name: "english"; MessagesFile: "compiler:Default.isl"

[Tasks]
Name: "desktopicon"; Description: "{cm:CreateDesktopIcon}"; GroupDescription: "{cm:AdditionalIcons}"; Flags: unchecked

[Files]
; x64-only installer
#if MyArch == "x64"
Source: "..\target\x86_64-pc-windows-msvc\release\lotus-dm.exe"; DestDir: "{app}"; DestName: "LotusDM.exe"; Flags: ignoreversion
#endif
; x86-only installer
#if MyArch == "x86"
Source: "..\target\i686-pc-windows-msvc\release\lotus-dm.exe"; DestDir: "{app}"; DestName: "LotusDM.exe"; Flags: ignoreversion
#endif
; universal installer: pick the right binary for the machine at install time
#if MyArch == "both"
Source: "..\target\x86_64-pc-windows-msvc\release\lotus-dm.exe"; DestDir: "{app}"; DestName: "LotusDM.exe"; Flags: ignoreversion; Check: IsWin64
Source: "..\target\i686-pc-windows-msvc\release\lotus-dm.exe"; DestDir: "{app}"; DestName: "LotusDM.exe"; Flags: ignoreversion; Check: not IsWin64
#endif

[Icons]
Name: "{group}\Lotus Download Manager"; Filename: "{app}\LotusDM.exe"
Name: "{group}\{cm:UninstallProgram,Lotus Download Manager}"; Filename: "{uninstallexe}"
Name: "{autodesktop}\Lotus Download Manager"; Filename: "{app}\LotusDM.exe"; Tasks: desktopicon

[Run]
Filename: "{app}\LotusDM.exe"; Description: "{cm:LaunchProgram,Lotus Download Manager}"; Flags: nowait postinstall skipifsilent

[UninstallDelete]
; App state lives in %APPDATA%\LotusDM and the download folder; the uninstaller
; leaves both alone on purpose.
Type: dirifempty; Name: "{app}"
