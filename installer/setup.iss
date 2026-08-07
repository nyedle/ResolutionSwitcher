; Builds ResolutionSwitcher-Setup-<arch>.exe
;
;   iscc /DArch=x64   /DAppVersion=1.0.0 installer\setup.iss
;   iscc /DArch=arm64 /DAppVersion=1.0.0 installer\setup.iss
;
; Expects the matching binary at dist\<arch>\resolution-switcher.exe

#ifndef AppVersion
  #define AppVersion "1.0.0"
#endif
#ifndef Arch
  #define Arch "x64"
#endif

#define AppName "Resolution Switcher"
#define AppExe "resolution-switcher.exe"
#define AppUrl "https://github.com/nyedle/ResolutionSwitcher"

[Setup]
AppId={{7B4C1E2A-9D3F-4A61-B0C8-5E2F1A7D6C43}
AppName={#AppName}
AppVersion={#AppVersion}
AppPublisher=nyedle
AppPublisherURL={#AppUrl}
AppSupportURL={#AppUrl}/issues
AppUpdatesURL={#AppUrl}/releases
DefaultDirName={autopf}\{#AppName}
DefaultGroupName={#AppName}
UninstallDisplayIcon={app}\{#AppExe}
LicenseFile=..\LICENSE
OutputDir=..\dist
OutputBaseFilename=ResolutionSwitcher-Setup-{#Arch}
Compression=lzma2/max
SolidCompression=yes
WizardStyle=modern
; No admin needed. Changing your own display modes never has.
PrivilegesRequired=lowest
PrivilegesRequiredOverridesAllowed=dialog
#if Arch == "arm64"
ArchitecturesAllowed=arm64
ArchitecturesInstallIn64BitMode=arm64
#else
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
#endif
CloseApplications=yes
RestartApplications=no
DisableProgramGroupPage=yes

[Languages]
Name: "english"; MessagesFile: "compiler:Default.isl"

[Tasks]
Name: "desktopicon"; Description: "Create a desktop shortcut"; GroupDescription: "Shortcuts:"
Name: "startup"; Description: "Start with Windows (needed for hotkeys to work from a cold boot)"; GroupDescription: "Startup:"; Flags: unchecked

[Files]
Source: "..\dist\{#Arch}\{#AppExe}"; DestDir: "{app}"; Flags: ignoreversion

[Icons]
Name: "{group}\{#AppName}"; Filename: "{app}\{#AppExe}"
Name: "{autodesktop}\{#AppName}"; Filename: "{app}\{#AppExe}"; Tasks: desktopicon

[Registry]
Root: HKCU; Subkey: "Software\Microsoft\Windows\CurrentVersion\Run"; ValueType: string; \
    ValueName: "{#AppName}"; ValueData: """{app}\{#AppExe}"""; Flags: uninsdeletevalue; Tasks: startup

[Run]
Filename: "{app}\{#AppExe}"; Description: "Launch {#AppName}"; Flags: nowait postinstall skipifsilent

; Slots and settings live in %APPDATA%\ResolutionSwitcher and are deliberately
; left behind, so reinstalling doesn't wipe someone's hotkeys.
