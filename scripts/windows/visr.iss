; Inno Setup script for the Visr installer; it needs Inno Setup 6.3 or later (x64compatible).
; scripts/windows/build-installer.ps1 sets the environment variables below and runs it.
; The layout follows the Sonora installer (GPL-3.0-or-later, same as Visr):
; https://github.com/atleugim/sonora, scripts/windows/sonora.iss. Only the structure was
; adapted; its file associations, updater and relaunch logic are not used.

#define AppName "Visr"
#define AppExeName "visr.exe"
#define AppVersion GetEnv("VISR_VERSION")
#define SourceExe GetEnv("VISR_EXE")
#define OutputDir GetEnv("VISR_DIST")
; x64compatible for the x64 build, arm64 for the ARM one
#define Arch GetEnv("VISR_ARCH")
; File name of the installer, without the .exe
#define SetupName GetEnv("VISR_SETUP")

[Setup]
; Identifies Visr to Windows across versions, so a new installer upgrades in place.
; Never change it.
AppId={{08EE7026-778F-492F-971E-C3B4B2D3AB3D}
AppName={#AppName}
AppVersion={#AppVersion}
AppPublisher={#AppName}
DefaultDirName={localappdata}\Programs\{#AppName}
DisableProgramGroupPage=yes
UninstallDisplayIcon={app}\{#AppExeName}
OutputDir={#OutputDir}
OutputBaseFilename={#SetupName}
Compression=lzma2
SolidCompression=yes
ArchitecturesAllowed={#Arch}
ArchitecturesInstallIn64BitMode={#Arch}
; Per-user only: no administrator prompt, and no option to install for all users.
PrivilegesRequired=lowest
WizardStyle=modern

[Files]
Source: "{#SourceExe}"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\..\target\licenses\*"; DestDir: "{app}\licenses"; Flags: ignoreversion recursesubdirs

[Icons]
Name: "{autoprograms}\{#AppName}"; Filename: "{app}\{#AppExeName}"

[Run]
Filename: "{app}\{#AppExeName}"; Description: "Launch Visr"; Flags: nowait postinstall skipifsilent
