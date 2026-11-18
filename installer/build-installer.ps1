$ErrorActionPreference = 'Stop'

$installerDir = $PSScriptRoot
$desktopDir = Split-Path -Parent $installerDir
$sourceExe = Join-Path $desktopDir 'target-dashboard\release\matto-chales-desktop.exe'
$outputDir = Join-Path $desktopDir 'dist'
$outputExe = Join-Path $outputDir 'MattoChales-Setup-0.1.41.exe'
$workDir = Join-Path $installerDir 'build'
$csc = Join-Path $env:WINDIR 'Microsoft.NET\Framework64\v4.0.30319\csc.exe'
$iconPath = Join-Path $desktopDir 'assets\matto-chales.ico'
$setIconScript = Join-Path $installerDir 'set-exe-icon.ps1'

if (-not (Test-Path -LiteralPath $sourceExe)) { throw "Executavel release nao encontrado: $sourceExe" }
if (-not (Test-Path -LiteralPath $csc)) { throw "Compilador C# do Windows nao encontrado: $csc" }

. $setIconScript -ExecutablePath $sourceExe -IconPath $iconPath

New-Item -ItemType Directory -Force -Path $outputDir, $workDir | Out-Null
if (Test-Path -LiteralPath $outputExe) { Remove-Item -LiteralPath $outputExe -Force }

$arguments = @(
    '/nologo', '/target:winexe', '/platform:anycpu', '/optimize+', '/codepage:65001',
    '/out:desktop-rust\dist\MattoChales-Setup-0.1.41.exe',
    '/win32icon:desktop-rust\assets\matto-chales.ico',
    '/reference:System.dll',
    '/reference:System.Drawing.dll',
    '/reference:System.Windows.Forms.dll',
    '/reference:Microsoft.CSharp.dll',
    '/resource:desktop-rust\target-dashboard\release\matto-chales-desktop.exe,MattoChales.exe',
    '/resource:desktop-rust\assets\installer-wallpaper.jpg,InstallerWallpaper.jpg',
    '/resource:desktop-rust\assets\app-icon.png,ChaletMark.png',
    '/resource:desktop-rust\assets\matto-chales.ico,MattoChales.ico',
    'desktop-rust\installer\Installer.cs'
)
Push-Location (Split-Path -Parent $desktopDir)
& $csc @arguments
Pop-Location
if ($LASTEXITCODE -ne 0 -or -not (Test-Path -LiteralPath $outputExe)) {
    throw "A compilacao do instalador falhou. Codigo: $LASTEXITCODE"
}

$installer = Get-Item -LiteralPath $outputExe
Write-Output "Instalador criado: $($installer.FullName)"
Write-Output "Tamanho: $([math]::Round($installer.Length / 1MB, 2)) MB"
