; Lekha Tally Agent — Inno Setup installer script.
;
; What this produces:
;   installer_out\LekhaTallyAgentSetup.exe
; What it does when the user runs that .exe:
;   - Installs lekha_tally.exe to %LOCALAPPDATA%\Programs\Lekha\TallyAgent
;     (per-user install; no admin prompt; works on locked-down corporate PCs)
;   - Creates a Start Menu shortcut under "Lekha"
;   - Sets HKCU\...\Run so the agent auto-starts when the user logs in
;   - Registers an uninstaller in Settings > Apps
;   - Offers a "Launch now" checkbox at the end of the install
;
; Build:
;   "C:\Users\mishr\AppData\Local\Programs\Inno Setup 6\ISCC.exe" installer\lekha_tally_agent.iss
;
; Inputs:
;   ..\target\release\lekha_tally.exe   (cargo build --release must have been run)

#define MyAppName         "Lekha Tally Agent"
#define MyAppVersion      "0.1.0"
#define MyAppPublisher    "Lekha AI"
#define MyAppURL          "https://lekha.ai"
#define MyAppExeName      "lekha_tally.exe"

[Setup]
; A unique GUID for this app — Windows uses this to track installs/upgrades.
; Don't change this between versions, or every version looks like a different app.
AppId={{D7E9F8A1-4B2C-4E3D-9F8A-LEKHATALLY01}}
AppName={#MyAppName}
AppVersion={#MyAppVersion}
AppPublisher={#MyAppPublisher}
AppPublisherURL={#MyAppURL}
AppSupportURL={#MyAppURL}
AppUpdatesURL={#MyAppURL}

; Per-user install — no admin prompt, no UAC, lands in user's AppData.
; (Same pattern as VS Code's per-user installer.)
PrivilegesRequired=lowest
DefaultDirName={userappdata}\Programs\Lekha\TallyAgent
DefaultGroupName=Lekha
DisableProgramGroupPage=yes
DisableDirPage=auto

; Output location for the built .exe installer.
OutputDir=..\installer_out
OutputBaseFilename=LekhaTallyAgentSetup
SetupIconFile=

; Compression
Compression=lzma2
SolidCompression=yes
ArchitecturesInstallIn64BitMode=x64compatible
ArchitecturesAllowed=x64compatible

; Wizard look
WizardStyle=modern
ShowLanguageDialog=no

[Languages]
Name: "english"; MessagesFile: "compiler:Default.isl"

[Tasks]
Name: "autostart"; Description: "Start &Lekha Tally Agent automatically when I sign in"; \
    GroupDescription: "Additional options:"

[Files]
; The agent binary. Relative to the .iss file location (installer\).
Source: "..\target\release\{#MyAppExeName}"; DestDir: "{app}"; Flags: ignoreversion

[Icons]
Name: "{group}\{#MyAppName}";   Filename: "{app}\{#MyAppExeName}"
Name: "{group}\Uninstall {#MyAppName}"; Filename: "{uninstallexe}"

[Registry]
; Auto-start on login — per-user HKCU Run key. Only added if the user kept
; the "autostart" task checkbox ticked. uninsdeletevalue ensures uninstall
; removes the key.
Root: HKCU; Subkey: "Software\Microsoft\Windows\CurrentVersion\Run"; \
    ValueType: string; ValueName: "LekhaTallyAgent"; \
    ValueData: """{app}\{#MyAppExeName}"""; \
    Flags: uninsdeletevalue; Tasks: autostart

[Run]
; Offer to launch the agent immediately after install.
Filename: "{app}\{#MyAppExeName}"; Description: "&Launch {#MyAppName} now"; \
    Flags: nowait postinstall skipifsilent

[UninstallRun]
; On uninstall, terminate any running instance of the agent so its files can
; be deleted. /F = force, /IM = image name.
Filename: "taskkill.exe"; Parameters: "/F /IM {#MyAppExeName}"; \
    Flags: runhidden; RunOnceId: "KillAgent"
