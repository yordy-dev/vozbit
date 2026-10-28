param(
    [string]$ReleaseTag = 'v0.5.8'
)

$ErrorActionPreference = 'Stop'
$repository = 'yordy-dev/vozbit'
$root = Split-Path -Parent $PSScriptRoot
$modelDirectory = Join-Path $root 'app/src-tauri/resources/models'
$models = @(
    @{
        Name = 'ggml-base-q5_1.bin'
        Sha256 = '422F1AE452ADE6F30A004D7E5C6A43195E4433BC370BF23FAC9CC591F01A8898'
    },
    @{
        Name = 'ultra-q8_0.gguf'
        Sha256 = 'C2FB452A9DF468A141012B01C8C168A25CE93F710897C7DE6E353C6CC250986A'
    }
)

New-Item -ItemType Directory -Path $modelDirectory -Force | Out-Null
foreach ($model in $models) {
    $destination = Join-Path $modelDirectory $model.Name
    if (Test-Path -LiteralPath $destination) {
        $existingHash = (Get-FileHash -LiteralPath $destination -Algorithm SHA256).Hash
        if ($existingHash -eq $model.Sha256) {
            Write-Host "$($model.Name): ya está verificado."
            continue
        }
    }

    $temporaryFile = "$destination.download"
    $assetUrl = "https://github.com/$repository/releases/download/$ReleaseTag/$($model.Name)"
    try {
        Write-Host "Descargando $($model.Name) desde $ReleaseTag..."
        Invoke-WebRequest -Uri $assetUrl -OutFile $temporaryFile
        $downloadedHash = (Get-FileHash -LiteralPath $temporaryFile -Algorithm SHA256).Hash
        if ($downloadedHash -ne $model.Sha256) {
            throw "El SHA-256 de $($model.Name) no coincide."
        }
        Move-Item -LiteralPath $temporaryFile -Destination $destination -Force
        Write-Host "$($model.Name): descarga verificada."
    }
    finally {
        if (Test-Path -LiteralPath $temporaryFile) {
            Remove-Item -LiteralPath $temporaryFile -Force
        }
    }
}
