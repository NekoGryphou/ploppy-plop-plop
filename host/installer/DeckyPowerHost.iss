#define AppName "DeckyPowerHost"
#define AppVersion "0.1.0"
#define HostPort "47991"

[Setup]
AppId={{F90FA28A-1710-43CD-BC1A-A5AA2AC2DC39}
AppName={#AppName}
AppVersion={#AppVersion}
DefaultDirName={autopf}\DeckyPowerHost
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
PrivilegesRequired=admin
OutputDir=..\..\out\host
OutputBaseFilename=DeckyPowerHost-Setup
Compression=lzma2
SolidCompression=yes
UninstallDisplayName=DeckyPowerHost
SetupLogging=yes

[Files]
Source: "..\target\x86_64-pc-windows-msvc\release\decky-power-host.exe"; DestDir: "{app}"; DestName: "DeckyPowerHost.exe"; Flags: ignoreversion
Source: "..\..\out\control\*"; DestDir: "{app}"; Flags: ignoreversion recursesubdirs createallsubdirs
Source: "DeckyPowerHost.toml"; DestDir: "{app}"; Flags: onlyifdoesntexist uninsneveruninstall

[Run]
Filename: "{app}\DeckyPowerHostControl.exe"; Description: "Open Decky Power Host"; Flags: postinstall nowait skipifsilent runasoriginaluser

[Icons]
Name: "{group}\Decky Power Host"; Filename: "{app}\DeckyPowerHostControl.exe"; WorkingDir: "{app}"

[UninstallRun]
Filename: "{sys}\sc.exe"; Parameters: "stop DeckyPowerHost"; Flags: runhidden waituntilterminated; RunOnceId: "StopService"
Filename: "{sys}\sc.exe"; Parameters: "delete DeckyPowerHost"; Flags: runhidden waituntilterminated; RunOnceId: "DeleteService"
Filename: "{sys}\netsh.exe"; Parameters: "advfirewall firewall delete rule name=""DeckyPowerHost"""; Flags: runhidden waituntilterminated; RunOnceId: "DeleteFirewall"

[Code]
var ConfiguredPort: String;

function ServiceExists: Boolean;
var ResultCode: Integer;
begin
  Result := Exec(ExpandConstant('{sys}\sc.exe'), 'query DeckyPowerHost', '', SW_HIDE, ewWaitUntilTerminated, ResultCode) and (ResultCode = 0);
end;

procedure ExecRequired(const Filename, Parameters, Description: String);
var ResultCode: Integer;
begin
  if not Exec(Filename, Parameters, '', SW_HIDE, ewWaitUntilTerminated, ResultCode) or (ResultCode <> 0) then
    RaiseException(Description + ' failed (exit code ' + IntToStr(ResultCode) + '). Setup stopped without reporting a successful installation.');
end;

function ReadPortFromConfig: String;
var Lines: TArrayOfString; Index, EqualsAt: Integer; Line, Value: String;
begin
  Result := '{#HostPort}';
  if not LoadStringsFromFile(ExpandConstant('{app}\DeckyPowerHost.toml'), Lines) then Exit;
  for Index := 0 to GetArrayLength(Lines) - 1 do begin
    Line := Trim(Lines[Index]);
    EqualsAt := Pos('=', Line);
    if (EqualsAt > 0) and (Trim(Copy(Line, 1, EqualsAt - 1)) = 'port') then begin
      Value := Trim(Copy(Line, EqualsAt + 1, Length(Line)));
      if (StrToIntDef(Value, 0) >= 1) and (StrToIntDef(Value, 0) <= 65535) then Result := Value;
      Exit;
    end;
  end;
end;

function GetConfiguredPort(Param: String): String;
begin
  if ConfiguredPort = '' then ConfiguredPort := ReadPortFromConfig;
  Result := ConfiguredPort;
end;

procedure CurStepChanged(CurStep: TSetupStep);
var ResultCode: Integer;
begin
  if (CurStep = ssInstall) and ServiceExists then begin
    { Stop before replacing the executable; sc.exe returns success while the
      service transitions, so wait for the process image to be released. }
    if not Exec(ExpandConstant('{sys}\sc.exe'), 'stop DeckyPowerHost', '', SW_HIDE, ewWaitUntilTerminated, ResultCode) or
       ((ResultCode <> 0) and (ResultCode <> 1062)) then
      RaiseException('Stopping the existing DeckyPowerHost service failed (exit code ' + IntToStr(ResultCode) + ').');
    Sleep(1500);
  end;
  if CurStep = ssPostInstall then begin
    if not ForceDirectories(ExpandConstant('{commonappdata}\DeckyPowerHost')) then
      RaiseException('DeckyPowerHost could not create its protected credential directory.');
    if not Exec(ExpandConstant('{sys}\icacls.exe'), '"' + ExpandConstant('{commonappdata}\DeckyPowerHost') + '" /inheritance:r /grant:r *S-1-5-18:(OI)(CI)F *S-1-5-32-544:(OI)(CI)F', '', SW_HIDE, ewWaitUntilTerminated, ResultCode) or (ResultCode <> 0) then
      RaiseException('DeckyPowerHost could not secure its credential directory.');
    if ServiceExists then
      ExecRequired(ExpandConstant('{sys}\sc.exe'), 'config DeckyPowerHost binPath= ""' + ExpandConstant('{app}\DeckyPowerHost.exe') + '" --service" start= auto', 'Configuring the DeckyPowerHost service')
    else
      ExecRequired(ExpandConstant('{sys}\sc.exe'), 'create DeckyPowerHost binPath= ""' + ExpandConstant('{app}\DeckyPowerHost.exe') + '" --service" start= auto DisplayName= "DeckyPowerHost"', 'Creating the DeckyPowerHost service');
    ExecRequired(ExpandConstant('{sys}\sc.exe'), 'failure DeckyPowerHost reset= 86400 actions= restart/5000/restart/15000', 'Configuring service recovery');
    { Deleting an absent rule may return a nonzero result, so only creation is fatal. }
    Exec(ExpandConstant('{sys}\netsh.exe'), 'advfirewall firewall delete rule name="DeckyPowerHost"', '', SW_HIDE, ewWaitUntilTerminated, ResultCode);
    ExecRequired(ExpandConstant('{sys}\netsh.exe'), 'advfirewall firewall add rule name="DeckyPowerHost" dir=in action=allow protocol=TCP localport=' + GetConfiguredPort('') + ' profile=private program="' + ExpandConstant('{app}\DeckyPowerHost.exe') + '" enable=yes', 'Creating the private-network firewall rule');
    ExecRequired(ExpandConstant('{sys}\sc.exe'), 'start DeckyPowerHost', 'Starting the DeckyPowerHost service');
  end;
end;
