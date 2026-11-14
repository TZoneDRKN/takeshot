# Checks `takeshot --version` on Windows with the release exe (a GUI program with no console of its own,
# which `cargo test` cannot reach: the debug build is a console program).
#
#   powershell -NoProfile -File scripts\windows-version-check.ps1 <path to takeshot.exe> [<more of them>]
#
# With nothing redirected the line must reach the console of the shell that started the exe, so each
# of those cases opens a cmd window for a few seconds and reads what its screen buffer shows. With
# the output redirected to a file or through a pipe the line must reach those. Nothing may be left
# running, and the settings and the log folder must stay as they were. It refuses to start while a
# process with the exe's name is running (the user's Takeshot, say), and exits 1 on any failure.
param(
    [Parameter(Mandatory, Position = 0)][string]$Exe,
    # Internal: run this command line in a new console, read its screen buffer and print it.
    [string]$ReadConsoleOf
)
$ErrorActionPreference = 'Stop'

if ($ReadConsoleOf) {
    Add-Type -TypeDefinition @'
using System;
using System.Runtime.InteropServices;
using System.Text;
public static class Con {
    [DllImport("kernel32.dll", SetLastError = true)] public static extern bool FreeConsole();
    [DllImport("kernel32.dll", SetLastError = true)] public static extern bool AttachConsole(uint pid);
    [DllImport("kernel32.dll", CharSet = CharSet.Unicode, SetLastError = true)]
    public static extern IntPtr CreateFileW(string name, uint access, uint share, IntPtr sec, uint disp, uint flags, IntPtr tmpl);
    [StructLayout(LayoutKind.Sequential)] public struct COORD { public short X; public short Y; }
    [DllImport("kernel32.dll", CharSet = CharSet.Unicode, SetLastError = true)]
    public static extern bool ReadConsoleOutputCharacter(IntPtr h, StringBuilder sb, uint len, COORD at, out uint read);
}
'@
    # cmd /k keeps the console open after the command, so its buffer can be read; echo marks the end.
    $cmd = Start-Process -FilePath cmd.exe -ArgumentList ('/s /k "' + $ReadConsoleOf + ' & echo [end]"') -PassThru
    Start-Sleep -Seconds 4
    [void][Con]::FreeConsole()
    if (-not [Con]::AttachConsole([uint32]$cmd.Id)) { Write-Output "could not attach to the console of $($cmd.Id)"; exit 2 }
    # Our own standard output is the pipe to the caller: the console's buffer is opened by name.
    $buffer = [Con]::CreateFileW('CONOUT$', [uint32]3221225472, 3, [IntPtr]::Zero, 3, 0, [IntPtr]::Zero)
    for ($y = 0; $y -lt 12; $y++) {
        $text = New-Object System.Text.StringBuilder 200
        [uint32]$read = 0
        $at = New-Object Con+COORD
        $at.Y = [int16]$y
        if (-not [Con]::ReadConsoleOutputCharacter($buffer, $text, 200, $at, [ref]$read)) { break }
        $row = ($text.ToString() -replace '\s{2,}', '   ').TrimEnd()
        if ($row -ne '') { Write-Output "| $row" }
    }
    [void][Con]::FreeConsole()
    Stop-Process -Id $cmd.Id -Force
    exit 0
}

$Exe = (Resolve-Path $Exe).Path
$name = [IO.Path]::GetFileNameWithoutExtension($Exe)
$cargo = Join-Path $PSScriptRoot '..\Cargo.toml'
$version = (Select-String -Path $cargo -Pattern '^version = "(.+)"' | Select-Object -First 1).Matches[0].Groups[1].Value
$want = "takeshot $version"
$settings = Join-Path $env:APPDATA 'takeshot\settings.json'
$logs = Join-Path $env:LOCALAPPDATA 'takeshot\logs'
function Running { (Get-Process -Name $name -ErrorAction SilentlyContinue | Measure-Object).Count }
function SettingsSum { if (Test-Path $settings) { (Get-FileHash $settings -Algorithm SHA256).Hash } else { 'none' } }
function LogStamp {
    if (-not (Test-Path $logs)) { return 'none' }
    (Get-ChildItem $logs | ForEach-Object { "$($_.Name):$($_.LastWriteTimeUtc.Ticks)" }) -join ','
}
if ((Running) -ne 0) { throw "a $name is running: quit it first" }
$before = @{ settings = SettingsSum; logs = LogStamp }
$scratch = Join-Path ([IO.Path]::GetTempPath()) "takeshot-version-check-$PID"
New-Item -ItemType Directory -Path $scratch | Out-Null

$script:failed = $false
function Check($what, $got, $expected) {
    $ok = ($got -eq $expected)
    Write-Output ('{0,-4} {1}' -f ($(if ($ok) { 'ok' } else { 'FAIL' })), $what)
    if (-not $ok) { Write-Output "       got:      '$got'"; Write-Output "       expected: '$expected'"; $script:failed = $true }
}

foreach ($flag in '--version', '-V') {
    $out = Join-Path $scratch "out$flag.txt"
    $p = Start-Process -FilePath $Exe -ArgumentList $flag -Wait -PassThru -RedirectStandardOutput $out -RedirectStandardError (Join-Path $scratch 'err.txt')
    Check "$flag to a file: exit 0" $p.ExitCode 0
    Check "$flag to a file: the line" ((Get-Content $out -Raw) -replace "`r`n", "`n") "$want`n"
}
Check 'through a pipe: the line' ((& $Exe --version | Out-String) -replace "`r`n", "`n") "$want`n"

# A GUI program is not waited for, so the shell's own `echo [end]` can land on the same row.
$exeLine = '"' + $Exe + '" --version'
foreach ($redirect in '', '2>nul', '<nul', '<nul 2>nul') {
    $screen = & powershell -NoProfile -File $PSCommandPath $Exe -ReadConsoleOf "$exeLine $redirect"
    $shown = ($screen | Where-Object { $_ -match ('^\| ' + [regex]::Escape($want) + '(\s|$)') } | Measure-Object).Count
    Check "console, nothing redirected for the output ($redirect): the line" $shown 1
}

Start-Sleep -Seconds 1
if ((Running) -ne 0) { Get-Process -Name $name | Stop-Process -Force }
Check 'nothing left running' (Running) 0
Check 'settings.json untouched' (SettingsSum) $before.settings
Check 'no log written or touched' (LogStamp) $before.logs
Remove-Item -Recurse -Force $scratch
if ($script:failed) { exit 1 }
Write-Output 'all ok'
