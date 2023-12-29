# PowerShell install script - installs the latest version of cargo-shuttle

Write-Host @"
     _           _   _   _
 ___| |__  _   _| |_| |_| | ___
/ __| '_ \| | | | __| __| |/ _ \
\__ \ | | | |_| | |_| |_| |  __/
|___/_| |_|\__,_|\__|\__|_|\___|

https://www.shuttle.rs
https://github.com/shuttle-hq/shuttle

Please file an issue if you encounter any problems!
===================================================
"@

$ErrorActionPreference = "Stop"

if (Get-Command -CommandType Application -ErrorAction SilentlyContinue cargo-binstall.exe) {
	Write-Host "Installing cargo-shuttle using cargo binstall"
	cargo-binstall.exe cargo-shuttle -y
	if ($?) {
		Write-Host "cargo-shuttle installed" -ForegroundColor Green
		[Environment]::Exit(0)
	}
	else {
		Write-Host "Could not install from release using cargo binstall, trying manual binary install" -ForegroundColor Red
	}
}
else {
	Write-Host "cargo binstall not found, trying manual binary install" -ForegroundColor Red
}

$CargoHome = if ($null -ne $Env:CARGO_HOME) { $Env:CARGO_HOME } else { "$HOME\.cargo" }
$RepoUrl = "https://github.com/shuttle-hq/shuttle"
$TempDir = $Env:TEMP
$Arch = [Environment]::GetEnvironmentVariable("PROCESSOR_ARCHITECTURE", [EnvironmentVariableTarget]::Machine)
if (($Arch -eq "AMD64") -and (Get-Command -CommandType Application -ErrorAction SilentlyContinue tar.exe)) {
	(Invoke-WebRequest "$RepoUrl/releases/latest" -Headers @{ "Accept" = "application/json" }).Content -match '"tag_name":"([^"]*)"' | Out-Null
	$LatestRelease = $Matches.1
	$BinaryUrl = "$RepoUrl/releases/download/$LatestRelease/cargo-shuttle-$LatestRelease-x86_64-pc-windows-msvc.tar.gz"
	Invoke-WebRequest $BinaryUrl -OutFile "$TempDir\cargo-shuttle.tar.gz"
	New-Item -ItemType Directory -Force "$TempDir\cargo-shuttle"
	tar.exe -xzf "$TempDir\cargo-shuttle.tar.gz" -C "$TempDir\cargo-shuttle"
	Move-Item "$TempDir\cargo-shuttle\cargo-shuttle-x86_64-pc-windows-msvc-$LatestRelease\cargo-shuttle.exe" "$CargoHome\bin\cargo-shuttle.exe"
	Remove-Item -Recurse -Force -ErrorAction SilentlyContinue "$TempDir\cargo-shuttle.tar.gz", "$TempDir\cargo-shuttle"
	Write-Host "cargo-shuttle installed" -ForegroundColor Green
	[Environment]::Exit(0)
}
elseif ($Arch -ne "AMD64") {
	Write-Host "Unsupported Architecture: Binaries are not currently built for $Arch, skipping manual binary install" -ForegroundColor Red
}
else {
	Write-Host "tar.exe not found, skipping manual binary install" -ForegroundColor Red
}

if (Get-Command -CommandType Application -ErrorAction SilentlyContinue cargo.exe) {
	cargo.exe install cargo-shuttle --locked
	if ($?) {
		Write-Host "cargo-shuttle installed" -ForegroundColor Green
		[Environment]::Exit(0)
	}
	else {
		Write-Host "Could not install cargo-shuttle using cargo" -ForegroundColor Red
		[Environment]::Exit(1)
	}
}
else {
	if ($Arch -in "AMD64", "x86") {
		Write-Host "cargo.exe not found" -ForegroundColor Red
		$Confirm = Read-Host -Prompt "Would you like to install Rust via Rustup? [y/N]"
		if ($Confirm -notin "y", "yes") {
			Write-Host "Skipping rustup install, cargo-shuttle not installed"
			[Environment]::Exit(1)
		}
		$RustupUrl = if ($Arch -eq "AMD64") { "https://win.rustup.rs/x86_64" } else { "https://win.rustup.rs/i686" }
		Invoke-WebRequest $RustupUrl -OutFile "$TempDir\rustup.exe"
		& "$TempDir\rustup.exe" toolchain install stable
		if ($?) {
			Remove-Item -ErrorAction SilentlyContinue "$TempDir\rustup.exe"
			Write-Host "Rust installed via Rustup, please re-run this script" -ForegroundColor Green
			[Environment]::Exit(0)
		}
		else {
			Remove-Item -ErrorAction SilentlyContinue "$TempDir\rustup.exe"
			Write-Host "Rust install via Rustup failed, please install Rust manually: https://rustup.rs/" -ForegroundColor Red
			[Environment]::Exit(0)
		}
	}
	else {
		Write-Host "cargo.exe not found, rustup only supports x86 and x86_64, please install Rust manually" -ForegroundColor Red
		[Environment]::Exit(1)
	}
}
