#Requires -Version 7.0
<#
.SYNOPSIS
    Print the one crate a task should open.
.DESCRIPTION
    Reads crates.map.json. Does not scan the repository.
#>
[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)]
    [string]$Keyword
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

try {
    $root = Split-Path -Parent $PSScriptRoot
    $mapPath = Join-Path $root 'crates.map.json'
    $map = Get-Content -LiteralPath $mapPath -Raw | ConvertFrom-Json
    $needle = $Keyword.Trim().ToLowerInvariant()

    if ($needle.Length -lt 2) {
        throw "Keyword must be at least 2 characters."
    }

    $exact = @($map.crates | Where-Object {
            $_.id -eq $needle -or
            $_.package -eq $needle -or
            $_.package -eq "oxidepress-$needle" -or
            @($_.keywords) -contains $needle
        })

    $hits = $exact
    if ($hits.Count -eq 0) {
        $hits = @($map.crates | Where-Object {
                $_.summary.ToLowerInvariant().Contains($needle) -or
                @($_.keywords | Where-Object { $_.ToLowerInvariant().Contains($needle) }).Count -gt 0
            })
    }

    if ($hits.Count -eq 0) {
        $ids = @($map.crates | ForEach-Object { $_.id }) -join ', '
        throw "No crate matches '$Keyword'. Known ids: $ids"
    }

    foreach ($hit in $hits) {
        Write-Host "crate: $($hit.id)"
        Write-Host "package: $($hit.package)"
        Write-Host "path: $($hit.path)"
        Write-Host "summary: $($hit.summary)"
        Write-Host ("neighbors: " + (@($hit.neighbors) -join ', '))
        Write-Host "scoped:"
        Write-Host "  cargo check -p $($hit.package)"
        Write-Host "  cargo test -p $($hit.package) --lib"
        Write-Host "  cargo fmt -p $($hit.package) -- --check"
        Write-Host ""
    }

    if ($hits.Count -gt 1) {
        Write-Host "Multiple crates matched. Pick one before editing."
        Write-Host ""
    }

    Write-Host "before a PR:"
    foreach ($command in @($map.verify)) {
        Write-Host "  $command"
    }

    Write-Host ""
    Write-Host "do not change:"
    foreach ($item in @($map.denylist)) {
        Write-Host "  - $item"
    }

    [PSCustomObject]@{
        Keyword = $needle
        Crates  = @($hits | ForEach-Object { $_.id })
        Count   = $hits.Count
    }
}
catch {
    Write-Error $_
    exit 1
}
