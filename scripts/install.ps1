# Granary installation script for Windows
# Usage: irm https://raw.githubusercontent.com/speakeasy-api/granary/main/scripts/install.ps1 | iex

$ErrorActionPreference = 'Stop'

$Repo = "speakeasy-api/granary"
$BinaryName = "granary"
$InstallDir = if ($env:GRANARY_INSTALL_DIR) { $env:GRANARY_INSTALL_DIR } else { "$env:USERPROFILE\.granary\bin" }

function Write-Info {
    param([string]$Message)
    Write-Host "info: " -ForegroundColor Blue -NoNewline
    Write-Host $Message
}

function Write-Success {
    param([string]$Message)
    Write-Host "success: " -ForegroundColor Green -NoNewline
    Write-Host $Message
}

function Write-Warn {
    param([string]$Message)
    Write-Host "warning: " -ForegroundColor Yellow -NoNewline
    Write-Host $Message
}

function Write-Err {
    param([string]$Message)
    Write-Host "error: " -ForegroundColor Red -NoNewline
    Write-Host $Message
    exit 1
}

function Get-Architecture {
    $arch = [System.Environment]::GetEnvironmentVariable("PROCESSOR_ARCHITECTURE")
    switch ($arch) {
        "AMD64" { return "x86_64" }
        "x86" { Write-Err "32-bit Windows is not supported" }
        "ARM64" { Write-Err "ARM64 Windows is not yet supported" }
        default { Write-Err "Unknown architecture: $arch" }
    }
}

function Get-LatestVersion {
    $url = "https://api.github.com/repos/$Repo/releases"
    try {
        $releases = Invoke-RestMethod -Uri $url -Method Get -UseBasicParsing
        # Find first non-prerelease (stable) version
        foreach ($release in $releases) {
            if (-not $release.prerelease) {
                return $release.tag_name
            }
        }
        Write-Err "No stable releases found. Check https://github.com/$Repo/releases"
    }
    catch {
        Write-Err "Failed to get latest version. Check your internet connection or visit https://github.com/$Repo/releases"
    }
}

function Install-Granary {
    Write-Info "Installing granary..."

    $arch = Get-Architecture
    $target = "$arch-pc-windows-msvc"

    Write-Info "Detected platform: $target"

    # Use GRANARY_VERSION env var if set, otherwise fetch latest stable
    if ($env:GRANARY_VERSION) {
        $version = $env:GRANARY_VERSION
        # Add 'v' prefix if not present (GitHub tags use 'v' prefix)
        if (-not $version.StartsWith("v")) {
            $version = "v$version"
        }
        Write-Info "Installing requested version: $version"
    } else {
        $version = Get-LatestVersion
        Write-Info "Latest version: $version"
    }

    $archiveName = "$BinaryName-$target.zip"
    $downloadUrl = "https://github.com/$Repo/releases/download/$version/$archiveName"

    # Create temp directory
    $tempDir = New-Item -ItemType Directory -Path (Join-Path $env:TEMP ([System.Guid]::NewGuid().ToString()))

    try {
        $archivePath = Join-Path $tempDir $archiveName

        Write-Info "Downloading $archiveName..."

        # Use TLS 1.2
        [Net.ServicePointManager]::SecurityProtocol = [Net.SecurityProtocolType]::Tls12

        try {
            Invoke-WebRequest -Uri $downloadUrl -OutFile $archivePath -UseBasicParsing
        }
        catch {
            Write-Err "Failed to download from $downloadUrl"
        }

        Write-Info "Extracting..."
        Expand-Archive -Path $archivePath -DestinationPath $tempDir -Force

        # Create install directory if it doesn't exist
        if (!(Test-Path $InstallDir)) {
            New-Item -ItemType Directory -Path $InstallDir -Force | Out-Null
        }

        # Install binaries
        Write-Info "Installing to $InstallDir..."
        Copy-Item -Path (Join-Path $tempDir "$BinaryName.exe") -Destination (Join-Path $InstallDir "$BinaryName.exe") -Force
        Copy-Item -Path (Join-Path $tempDir "granaryd.exe") -Destination (Join-Path $InstallDir "granaryd.exe") -Force

        Write-Success "Granary $version installed successfully (granary + granaryd)!"

        # Check if install directory is in PATH
        $userPath = [Environment]::GetEnvironmentVariable("Path", "User")
        if ($userPath -notlike "*$InstallDir*") {
            Write-Warn "Adding $InstallDir to your PATH..."

            $newPath = "$userPath;$InstallDir"
            [Environment]::SetEnvironmentVariable("Path", $newPath, "User")

            Write-Info "PATH updated. Please restart your terminal for changes to take effect."
        }
        else {
            Write-Info "Installation directory is already in your PATH"
        }

        Write-Host ""
        Write-Info "Get started with: granary --help"
    }
    finally {
        # Cleanup temp directory
        Remove-Item -Path $tempDir -Recurse -Force -ErrorAction SilentlyContinue
    }
}

Install-Granary
