& {
    $ErrorActionPreference = 'Stop'

    function Install-GitDiffOut {
    $architecture = (Get-CimInstance Win32_Processor | Select-Object -First 1).Architecture
    if ($architecture -ne 9) {
        throw "error: unsupported Windows architecture: $architecture"
    }

    $version = $env:GD_VERSION
    if (-not $version) {
        $version = (Invoke-RestMethod 'https://api.github.com/repos/seapagan/git-diff-out/releases/latest').tag_name
        if (-not $version) { throw 'error: latest release tag was not found' }
    }

    $installDir = $env:GD_INSTALL_DIR
    if (-not $installDir) {
        $installDir = Join-Path $env:USERPROFILE '.local\bin'
    }

    $asset = "git-diff-out-v${version}-x86_64-pc-windows-msvc.zip"
    $url = "https://github.com/seapagan/git-diff-out/releases/download/${version}/${asset}"
    $tmpDir = Join-Path ([System.IO.Path]::GetTempPath()) ([System.IO.Path]::GetRandomFileName())
    New-Item -ItemType Directory -Path $tmpDir | Out-Null
    try {
        $archive = Join-Path $tmpDir $asset
        Invoke-WebRequest -Uri $url -OutFile $archive
        Expand-Archive -LiteralPath $archive -DestinationPath $tmpDir
        foreach ($name in 'gd.exe', 'git-diff-out.exe') {
            if (-not (Test-Path -LiteralPath (Join-Path $tmpDir $name) -PathType Leaf)) {
                throw "error: release archive is missing $name"
            }
        }

        New-Item -ItemType Directory -Force -Path $installDir | Out-Null
        foreach ($name in 'gd.exe', 'git-diff-out.exe') {
            Copy-Item -LiteralPath (Join-Path $tmpDir $name) -Destination (Join-Path $installDir $name) -Force
        }
    } finally {
        Remove-Item -LiteralPath $tmpDir -Recurse -Force
    }

    Write-Output "Installed git-diff-out $version to $installDir (gd.exe, git-diff-out.exe)."
    if (($env:PATH -split [System.IO.Path]::PathSeparator) -notcontains $installDir) {
        Write-Warning "$installDir is not on PATH; add it to PATH to use gd and git-diff-out."
    }
    }

    Install-GitDiffOut
}
