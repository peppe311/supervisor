; SPDX-License-Identifier: MPL-2.0
; Payload is intentionally enumerated. Never recurse over an application profile.
#ifndef PackageDirectory
  #error PackageDirectory is required
#endif
#ifndef OutputDirectory
  #error OutputDirectory is required
#endif
#ifndef ReleaseVersion
  #error ReleaseVersion is required
#endif
#ifndef NumericVersion
  #error NumericVersion is required
#endif
#ifndef OutputBaseName
  #error OutputBaseName is required
#endif
#ifndef InstallerAppId
  #define InstallerAppId "Supervisor.Desktop"
#endif
#ifndef ApplicationMutex
  #define ApplicationMutex "Local\Supervisor.CentralAgent.SingleInstance"
#endif

[Setup]
AppId={#InstallerAppId}
AppName=Supervisor
AppVersion={#ReleaseVersion}
AppVerName=Supervisor {#ReleaseVersion}
AppPublisher=Supervisor contributors
AppPublisherURL=https://github.com/peppe311/supervisor
AppSupportURL=https://github.com/peppe311/supervisor/issues
AppUpdatesURL=https://github.com/peppe311/supervisor/releases
VersionInfoVersion={#NumericVersion}
DefaultDirName={localappdata}\Programs\Supervisor
DefaultGroupName=Supervisor
PrivilegesRequired=lowest
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
MinVersion=10.0
AppMutex={#ApplicationMutex}
SetupMutex=Supervisor.Desktop.Installer
CloseApplications=no
RestartApplications=no
AllowNoIcons=yes
DisableProgramGroupPage=yes
WizardStyle=modern
WizardResizable=no
SetupIconFile=..\..\assets\supervisor-app-icon.ico
UninstallDisplayIcon={app}\Supervisor.exe
LicenseFile={#PackageDirectory}\LICENSE.txt
InfoBeforeFile=INSTALLER_NOTICE.txt
Compression=lzma2/normal
SolidCompression=yes
OutputDir={#OutputDirectory}
OutputBaseFilename={#OutputBaseName}
#ifdef SignedInstaller
SignTool=supervisor
SignedUninstaller=yes
#else
SignedUninstaller=no
#endif

[Languages]
Name: "english"; MessagesFile: "compiler:Default.isl"
Name: "italian"; MessagesFile: "compiler:Languages\Italian.isl"

[Tasks]
Name: "desktopicon"; Description: "{cm:CreateDesktopIcon}"; Flags: unchecked

[Files]
Source: "{#PackageDirectory}\Supervisor.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#PackageDirectory}\Supervisor.release.json"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#PackageDirectory}\Supervisor.dependencies.json"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#PackageDirectory}\LICENSE.txt"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#PackageDirectory}\THIRD_PARTY_NOTICES.md"; DestDir: "{app}"; Flags: ignoreversion

[Icons]
Name: "{group}\Supervisor"; Filename: "{app}\Supervisor.exe"; WorkingDir: "{userdocs}"
Name: "{autodesktop}\Supervisor"; Filename: "{app}\Supervisor.exe"; WorkingDir: "{userdocs}"; Tasks: desktopicon

[Run]
Filename: "{app}\Supervisor.exe"; WorkingDir: "{userdocs}"; Description: "{cm:LaunchProgram,Supervisor}"; Flags: nowait postinstall skipifsilent unchecked

[Code]
function WebView2Installed: Boolean;
var
  Version: String;
begin
  Result := (RegQueryStringValue(HKLM32,
    'Software\Microsoft\EdgeUpdate\Clients\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}', 'pv', Version)
    and (Version <> '') and (Version <> '0.0.0.0'));
  if not Result then
    Result := (RegQueryStringValue(HKCU,
      'Software\Microsoft\EdgeUpdate\Clients\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}', 'pv', Version)
      and (Version <> '') and (Version <> '0.0.0.0'));
end;

function PrepareToInstall(var NeedsRestart: Boolean): String;
begin
  Result := '';
  if not WebView2Installed then
    Result := 'Supervisor needs Microsoft Edge WebView2 Runtime. Install the Evergreen Runtime from https://developer.microsoft.com/microsoft-edge/webview2/ and run this installer again.';
end;

function InitializeUninstall: Boolean;
begin
  Result := not CheckForMutexes('{#ApplicationMutex}');
  if not Result then
    SuppressibleMsgBox('Supervisor is still running. Choose Close Supervisor from its notification-area icon, then uninstall again.', mbError, MB_OK, IDOK);
end;

// No UninstallDelete section: never remove chats, credentials, projects or profiles.
