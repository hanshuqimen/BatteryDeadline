param([string]$OutputDirectory = '.artifacts/release')
$ErrorActionPreference = 'Stop'
$projectRoot = Split-Path -Parent $PSScriptRoot
$version = (Get-Content -Raw -LiteralPath (Join-Path $projectRoot 'package.json') | ConvertFrom-Json).version
$outputPath = [IO.Path]::GetFullPath((Join-Path $projectRoot $OutputDirectory))
New-Item -ItemType Directory -Force -Path $outputPath | Out-Null
$desktopPath = Join-Path $projectRoot 'target/release/battery-deadline.exe'
$cliPath = Join-Path $projectRoot 'target/release/battery-deadline-cli.exe'
$installerName = "BatteryDeadline_${version}_x64-setup.exe"
$installerPath = Join-Path $projectRoot "target/release/bundle/nsis/$installerName"
foreach ($requiredPath in @($desktopPath, $cliPath, $installerPath)) {
    if (-not (Test-Path -LiteralPath $requiredPath -PathType Leaf)) { throw "Build missing: $requiredPath" }
}
$portableDirectory = Join-Path $projectRoot '.artifacts/portable'
New-Item -ItemType Directory -Force -Path $portableDirectory | Out-Null
Copy-Item -LiteralPath $desktopPath, $cliPath -Destination $portableDirectory
Copy-Item -LiteralPath (Join-Path $projectRoot 'README.md'), (Join-Path $projectRoot 'README.zh-CN.md'), (Join-Path $projectRoot 'LICENSE') -Destination $portableDirectory
Copy-Item -LiteralPath (Join-Path $projectRoot 'CONTRIBUTING.md'), (Join-Path $projectRoot 'SECURITY.md'), (Join-Path $projectRoot 'CHANGELOG.md') -Destination $portableDirectory
Copy-Item -LiteralPath (Join-Path $projectRoot 'docs') -Destination $portableDirectory -Recurse -Force
Copy-Item -LiteralPath $installerPath -Destination $outputPath
$portableName = "BatteryDeadline_${version}_windows-x64-portable.zip"
Compress-Archive -Path (Join-Path $portableDirectory '*') -DestinationPath (Join-Path $outputPath $portableName) -Force
$checksumLines = foreach ($assetName in @($installerName, $portableName)) {
    $checksum = (Get-FileHash -Algorithm SHA256 -LiteralPath (Join-Path $outputPath $assetName)).Hash.ToLowerInvariant()
    "$checksum  $assetName"
}
Set-Content -LiteralPath (Join-Path $outputPath 'SHA256SUMS.txt') -Value $checksumLines -Encoding ascii
Get-ChildItem -LiteralPath $outputPath -File | Select-Object Name, Length
