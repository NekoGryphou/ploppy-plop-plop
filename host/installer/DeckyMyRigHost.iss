#define AppName "Decky My Rig Host"
#define AppVersion "0.1.0"
#define HostPort "47991"

[Setup]
AppId={{F90FA28A-1710-43CD-BC1A-A5AA2AC2DC39}
AppName={#AppName}
AppVersion={#AppVersion}
DefaultDirName={autopf}\DeckyMyRigHost
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
PrivilegesRequired=admin
OutputDir=..\..\out\host
OutputBaseFilename=DeckyMyRig_Host__Windows_x64
Compression=lzma2
SolidCompression=yes
UninstallDisplayName={#AppName}
SetupLogging=yes

[Files]
Source: "..\target\x86_64-pc-windows-msvc\release\decky-my-rig-host.exe"; DestDir: "{app}"; DestName: "DeckyMyRigHost.exe"; Flags: ignoreversion
Source: "..\..\out\control\*"; DestDir: "{app}"; Flags: ignoreversion recursesubdirs createallsubdirs
Source: "DeckyMyRigHost.toml"; DestDir: "{app}"; Flags: onlyifdoesntexist uninsneveruninstall

[Run]
Filename: "{app}\DeckyMyRigHostControl.exe"; Description: "Open Decky My Rig Host"; Flags: postinstall nowait skipifsilent runasoriginaluser

[Icons]
Name: "{group}\Decky My Rig Host"; Filename: "{app}\DeckyMyRigHostControl.exe"; WorkingDir: "{app}"

[UninstallRun]
Filename: "{sys}\sc.exe"; Parameters: "stop DeckyMyRigHost"; Flags: runhidden waituntilterminated; RunOnceId: "StopService"
Filename: "{sys}\sc.exe"; Parameters: "delete DeckyMyRigHost"; Flags: runhidden waituntilterminated; RunOnceId: "DeleteService"
Filename: "{sys}\netsh.exe"; Parameters: "advfirewall firewall delete rule name=""DeckyMyRigHost"""; Flags: runhidden waituntilterminated; RunOnceId: "DeleteFirewall"

[Code]
var ConfiguredPort: String;

function ServiceExists: Boolean;
var ResultCode: Integer;
begin
  Result := Exec(ExpandConstant('{sys}\sc.exe'), 'query DeckyMyRigHost', '', SW_HIDE, ewWaitUntilTerminated, ResultCode) and (ResultCode = 0);
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
  if not LoadStringsFromFile(ExpandConstant('{app}\DeckyMyRigHost.toml'), Lines) then Exit;
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
    if not Exec(ExpandConstant('{sys}\sc.exe'), 'stop DeckyMyRigHost', '', SW_HIDE, ewWaitUntilTerminated, ResultCode) or
       ((ResultCode <> 0) and (ResultCode <> 1062)) then
      RaiseException('Stopping the existing DeckyMyRigHost service failed (exit code ' + IntToStr(ResultCode) + ').');
    Sleep(1500);
  end;
  if CurStep = ssPostInstall then begin
    if not ForceDirectories(ExpandConstant('{commonappdata}\DeckyMyRigHost')) then
      RaiseException('DeckyMyRigHost could not create its protected credential directory.');
    if not Exec(ExpandConstant('{sys}\icacls.exe'), '"' + ExpandConstant('{commonappdata}\DeckyMyRigHost') + '" /inheritance:r /grant:r *S-1-5-18:(OI)(CI)F *S-1-5-32-544:(OI)(CI)F', '', SW_HIDE, ewWaitUntilTerminated, ResultCode) or (ResultCode <> 0) then
      RaiseException('DeckyMyRigHost could not secure its credential directory.');
    if ServiceExists then
      ExecRequired(ExpandConstant('{sys}\sc.exe'), 'config DeckyMyRigHost binPath= ""' + ExpandConstant('{app}\DeckyMyRigHost.exe') + '" --service" start= auto', 'Configuring the DeckyMyRigHost service')
    else
      ExecRequired(ExpandConstant('{sys}\sc.exe'), 'create DeckyMyRigHost binPath= ""' + ExpandConstant('{app}\DeckyMyRigHost.exe') + '" --service" start= auto DisplayName= "DeckyMyRigHost"', 'Creating the DeckyMyRigHost service');
    ExecRequired(ExpandConstant('{sys}\sc.exe'), 'failure DeckyMyRigHost reset= 86400 actions= restart/5000/restart/15000', 'Configuring service recovery');
    { Deleting an absent rule may return a nonzero result, so only creation is fatal. }
    Exec(ExpandConstant('{sys}\netsh.exe'), 'advfirewall firewall delete rule name="DeckyMyRigHost"', '', SW_HIDE, ewWaitUntilTerminated, ResultCode);
    ExecRequired(ExpandConstant('{sys}\netsh.exe'), 'advfirewall firewall add rule name="DeckyMyRigHost" dir=in action=allow protocol=TCP localport=' + GetConfiguredPort('') + ' profile=private program="' + ExpandConstant('{app}\DeckyMyRigHost.exe') + '" enable=yes', 'Creating the private-network firewall rule');
    ExecRequired(ExpandConstant('{sys}\sc.exe'), 'start DeckyMyRigHost', 'Starting the DeckyMyRigHost service');
  end;
end;
