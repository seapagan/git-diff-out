$ErrorActionPreference = 'Stop'

function Assert($Condition, $Message) {
    if (-not $Condition) { throw $Message }
}

$root = Join-Path ([System.IO.Path]::GetTempPath()) ([System.IO.Path]::GetRandomFileName())
$install = Join-Path $root 'install'
$archiveRoot = Join-Path $root 'archive'
$zip = Join-Path $root 'release.zip'
$originalPath = $env:PATH
$originalVersion = $env:GD_VERSION
$originalInstallDir = $env:GD_INSTALL_DIR

function Get-CimInstance {
    [pscustomobject]@{ Architecture = $script:TestArchitecture }
}
function Invoke-RestMethod {
    param($Uri)
    Assert ($Uri -eq 'https://api.github.com/repos/seapagan/git-diff-out/releases/latest') 'wrong latest endpoint'
    $script:LatestCalls++
    [pscustomobject]@{ tag_name = '0.2.0' }
}
function Invoke-WebRequest {
    param($Uri, $OutFile)
    $script:AssetUrl = $Uri
    Copy-Item $script:Zip -Destination $OutFile
}

try {
    New-Item -ItemType Directory -Path $archiveRoot, $install -Force | Out-Null
    Set-Content (Join-Path $archiveRoot 'gd.exe') 'new gd'
    Set-Content (Join-Path $archiveRoot 'git-diff-out.exe') 'new git-diff-out'
    Compress-Archive -Path (Join-Path $archiveRoot '*.exe') -DestinationPath $zip
    $script:Zip = $zip
    $script:TestArchitecture = 9
    $script:LatestCalls = 0
    $env:GD_INSTALL_DIR = $install
    $env:GD_VERSION = ''
    $env:PATH = $originalPath

    $output = & { . "$PSScriptRoot/../../install.ps1" } 3>&1 | Out-String
    Assert ($script:LatestCalls -eq 1) 'latest release was not fetched'
    Assert ($script:AssetUrl -like '*/releases/download/0.2.0/git-diff-out-v0.2.0-x86_64-pc-windows-msvc.zip') 'wrong latest asset'
    Assert ($output -match 'add it to PATH') 'missing PATH warning'
    Assert ((Get-Content (Join-Path $install 'gd.exe')) -eq 'new gd') 'gd.exe was not installed'
    Assert ((Get-Content (Join-Path $install 'git-diff-out.exe')) -eq 'new git-diff-out') 'git-diff-out.exe was not installed'

    Set-Content (Join-Path $install 'gd.exe') 'old gd'
    Set-Content (Join-Path $install 'git-diff-out.exe') 'old git-diff-out'
    $env:GD_VERSION = '0.1.0'
    $script:LatestCalls = 0
    $env:PATH = "$originalPath$([System.IO.Path]::PathSeparator)$install"
    $output = & { . "$PSScriptRoot/../../install.ps1" } 3>&1 | Out-String
    Assert ($script:LatestCalls -eq 0) 'GD_VERSION fetched latest'
    Assert ($script:AssetUrl -like '*/releases/download/0.1.0/git-diff-out-v0.1.0-x86_64-pc-windows-msvc.zip') 'wrong override asset'
    Assert ($output -notmatch 'add it to PATH') 'PATH warning appeared for an existing entry'
    Assert ((Get-Content (Join-Path $install 'gd.exe')) -eq 'new gd') 'gd.exe was not replaced'
    Assert ((Get-Content (Join-Path $install 'git-diff-out.exe')) -eq 'new git-diff-out') 'git-diff-out.exe was not replaced'

    $script:TestArchitecture = 12
    $failed = $false
    try {
        . "$PSScriptRoot/../../install.ps1" | Out-Null
    } catch {
        Assert ($_.Exception.Message -match 'unsupported') 'unsupported architecture message missing'
        $failed = $true
    }
    Assert $failed 'unsupported ARM64 succeeded'

    Write-Output 'Windows installer tests passed'
} finally {
    $env:PATH = $originalPath
    $env:GD_VERSION = $originalVersion
    $env:GD_INSTALL_DIR = $originalInstallDir
    Remove-Item $root -Recurse -Force -ErrorAction SilentlyContinue
}
