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
Source: "DeckyPowerHost.toml"; DestDir: "{app}"; Flags: onlyifdoesntexist uninsneveruninstall

[Run]
Filename: "{sys}\sc.exe"; Parameters: "create DeckyPowerHost binPath= ""{app}\DeckyPowerHost.exe"" start= auto DisplayName= ""DeckyPowerHost"""; Flags: runhidden waituntilterminated; Check: not ServiceExists
Filename: "{sys}\sc.exe"; Parameters: "config DeckyPowerHost binPath= ""{app}\DeckyPowerHost.exe"" start= auto"; Flags: runhidden waituntilterminated; Check: ServiceExists
Filename: "{sys}\sc.exe"; Parameters: "failure DeckyPowerHost reset= 86400 actions= restart/5000/restart/15000"; Flags: runhidden waituntilterminated
Filename: "{sys}\netsh.exe"; Parameters: "advfirewall firewall delete rule name=""DeckyPowerHost"""; Flags: runhidden waituntilterminated
Filename: "{sys}\netsh.exe"; Parameters: "advfirewall firewall add rule name=""DeckyPowerHost"" dir=in action=allow protocol=TCP localport={code:GetConfiguredPort} profile=private program=""{app}\DeckyPowerHost.exe"" enable=yes"; Flags: runhidden waituntilterminated
Filename: "{sys}\sc.exe"; Parameters: "start DeckyPowerHost"; Flags: runhidden waituntilterminated

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

function JoinLines(Lines: TArrayOfString): String;
var Index: Integer;
begin
  Result := '';
  for Index := 0 to GetArrayLength(Lines) - 1 do begin
    if Result <> '' then Result := Result + #13#10;
    Result := Result + Lines[Index];
  end;
end;

procedure CurStepChanged(CurStep: TSetupStep);
var ResultCode: Integer; Output: TExecOutput;
begin
  if CurStep = ssPostInstall then begin
    ForceDirectories(ExpandConstant('{commonappdata}\DeckyPowerHost'));
    Exec(ExpandConstant('{sys}\icacls.exe'), '"' + ExpandConstant('{commonappdata}\DeckyPowerHost') + '" /inheritance:r /grant:r SYSTEM:(OI)(CI)F Administrators:(OI)(CI)F', '', SW_HIDE, ewWaitUntilTerminated, ResultCode);
    if ExecAndCaptureOutput(ExpandConstant('{app}\DeckyPowerHost.exe'), '--pairing-code', '', SW_SHOWNORMAL, ewWaitUntilTerminated, ResultCode, Output) and (ResultCode = 0) then
      MsgBox(JoinLines(Output.StdOut) + #13#10 + 'Enter this code when adding the PC in Decky. It expires after five minutes.', mbInformation, MB_OK);
  end;
end;
