#Requires -Version 7.0
<#
.SYNOPSIS
    Rebrand rustpress crate identity to OxidePress.
.DESCRIPTION
    Dry-run unless -Apply. Rewrites crate manifests and Rust sources, then
    git-moves crates/rustpress-* to crates/oxidepress-*.
    Keeps session keys, OxidePress-owned table names, the rustpress_version
    template key, and RUSTPRESS_* environment variables.
#>
[CmdletBinding()]
param(
    [switch]$Apply
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$NoticeMarker = 'Modified by Vabian124 on 2026-11-07 to rebrand as OxidePress.'
$RsNotice = @"
// $NoticeMarker
// Original work: RustPress by Ryuji Yasukochi.
// Licensed under the GNU General Public License, version 2.

"@
$TomlNotice = @"
# $NoticeMarker
# Original work: RustPress by Ryuji Yasukochi.
# Licensed under the GNU General Public License, version 2.

"@

function Protect-Token {
    param(
        [string]$Text,
        [string]$Token,
        [System.Collections.Generic.List[object]]$Slots
    )
    $slot = "«KEEP$($Slots.Count)»"
    $Slots.Add([PSCustomObject]@{ Slot = $slot; Token = $Token })
    return $Text.Replace($Token, $slot)
}

function Restore-Tokens {
    param(
        [string]$Text,
        [System.Collections.Generic.List[object]]$Slots
    )
    for ($i = $Slots.Count - 1; $i -ge 0; $i--) {
        $entry = $Slots[$i]
        $Text = $Text.Replace($entry.Slot, $entry.Token)
    }
    return $Text
}

function Convert-ForkText {
    param(
        [string]$Text,
        [bool]$IsRootManifest
    )

    $slots = [System.Collections.Generic.List[object]]::new()
    $Text = Protect-Token -Text $Text -Token 'Original work: RustPress by Ryuji Yasukochi.' -Slots $slots
    $Text = Protect-Token -Text $Text -Token 'rustpress_version' -Slots $slots
    $Text = Protect-Token -Text $Text -Token '"rustpress_migrations"' -Slots $slots
    $Text = Protect-Token -Text $Text -Token '"rustpress_cache"' -Slots $slots
    $Text = Protect-Token -Text $Text -Token 'rustpress_session' -Slots $slots

    $Text = $Text.Replace('rustpress-', 'oxidepress-')
    $Text = $Text.Replace('rustpress_', 'oxidepress_')
    $Text = $Text.Replace('RustPress', 'OxidePress')
    $Text = $Text.Replace('rustpress', 'oxidepress')
    $Text = Restore-Tokens -Text $Text -Slots $slots

    if ($IsRootManifest) {
        $versionPattern = [regex]::new('(?m)^version = "0\.1\.0"')
        $Text = $versionPattern.Replace($Text, 'version = "0.2.0"', 1)
        if ($Text -notmatch '(?m)^authors = ') {
            $licensePattern = [regex]::new('(?m)^(license = "GPL-2\.0")(\r?\n)')
            $Text = $licensePattern.Replace($Text, 'license = "GPL-2.0"${2}authors = ["Vabian124"]${2}', 1)
        }
        $description = 'description = "OxidePress - a WordPress-compatible CMS built in Rust. Independent fork of RustPress."'
        $descriptionPattern = [regex]::new('(?m)^description = ".*"')
        $Text = $descriptionPattern.Replace($Text, $description, 1)
    }

    return $Text
}

function Add-ChangeNotice {
    param(
        [string]$Text,
        [string]$Extension
    )
    if ($Text.Contains($NoticeMarker)) {
        return $Text
    }
    if ($Extension -eq '.rs') {
        return $RsNotice + $Text
    }
    if ($Extension -eq '.toml') {
        return $TomlNotice + $Text
    }
    return $Text
}

function Test-AllowedPath {
    param(
        [string]$Root,
        [string]$Path
    )
    $relative = [System.IO.Path]::GetRelativePath($Root, $Path).Replace('\', '/')
    if ($relative -eq 'Cargo.toml') {
        return
    }
    if ($relative.StartsWith('crates/rustpress-') -or $relative.StartsWith('crates/oxidepress-')) {
        return
    }
    throw "Refusing to edit $relative"
}

function Get-UnexpectedNames {
    param([string]$Text)
    $stripped = $Text.Replace('rustpress_session', '')
    $stripped = $stripped.Replace('rustpress_version', '')
    $stripped = $stripped.Replace('rustpress_cache', '')
    $stripped = $stripped.Replace('rustpress_migrations', '')
    $found = [regex]::Matches($stripped, 'rustpress')
    return @($found | ForEach-Object { $_.Value })
}

function Get-RenamePlan {
    param([string]$Root)

    $files = [System.Collections.Generic.List[string]]::new()
    $files.Add((Join-Path $Root 'Cargo.toml'))

    $crateRoot = Join-Path $Root 'crates'
    $directories = @(Get-ChildItem -LiteralPath $crateRoot -Directory | Where-Object {
            $_.Name -like 'rustpress-*' -or $_.Name -like 'oxidepress-*'
        })

    foreach ($directory in $directories) {
        $sources = @(Get-ChildItem -LiteralPath $directory.FullName -Recurse -File | Where-Object {
                $_.Extension -in @('.rs', '.toml') -and $_.FullName -notmatch '[\\/]target[\\/]'
            })
        foreach ($source in $sources) {
            $files.Add($source.FullName)
        }
    }

    $moves = @(Get-ChildItem -LiteralPath $crateRoot -Directory | Where-Object {
            $_.Name -like 'rustpress-*'
        } | ForEach-Object {
            [PSCustomObject]@{
                From = "crates/$($_.Name)"
                To   = "crates/$($_.Name -replace '^rustpress-', 'oxidepress-')"
            }
        })

    $edits = [System.Collections.Generic.List[object]]::new()
    $unexpected = [System.Collections.Generic.List[string]]::new()

    foreach ($path in $files) {
        Test-AllowedPath -Root $Root -Path $path
        $original = [System.IO.File]::ReadAllText($path)
        $relative = [System.IO.Path]::GetRelativePath($Root, $path).Replace('\', '/')
        $updated = Convert-ForkText -Text $original -IsRootManifest ($relative -eq 'Cargo.toml')
        $leftovers = @(Get-UnexpectedNames -Text $updated)
        if ($leftovers.Count -gt 0) {
            $unexpected.Add("$relative ($($leftovers.Count))")
        }
        if ($updated -ne $original) {
            $updated = Add-ChangeNotice -Text $updated -Extension ([System.IO.Path]::GetExtension($path))
            $edits.Add([PSCustomObject]@{
                    Path     = $path
                    Relative = $relative
                    Updated  = $updated
                })
        }
    }

    [PSCustomObject]@{
        Edits      = $edits
        Moves      = $moves
        Unexpected = $unexpected
    }
}

try {
    $root = Split-Path -Parent $PSScriptRoot
    $plan = Get-RenamePlan -Root $root

    if ($plan.Unexpected.Count -gt 0) {
        $sample = ($plan.Unexpected | Select-Object -First 20) -join "`n  "
        throw "Unexpected rustpress leftovers:`n  $sample"
    }

    Write-Host ("mode: " + $(if ($Apply) { 'apply' } else { 'dry-run' }))
    Write-Host "files: $($plan.Edits.Count)"
    Write-Host "moves: $($plan.Moves.Count)"
    foreach ($move in $plan.Moves) {
        Write-Host "  git mv $($move.From) $($move.To)"
    }

    if (-not $Apply) {
        [PSCustomObject]@{
            Mode  = 'dry-run'
            Files = $plan.Edits.Count
            Moves = $plan.Moves.Count
        }
        exit 0
    }

    $utf8 = [System.Text.UTF8Encoding]::new($false)
    foreach ($edit in $plan.Edits) {
        [System.IO.File]::WriteAllText($edit.Path, $edit.Updated, $utf8)
    }

    foreach ($move in $plan.Moves) {
        & git -C $root mv -- $move.From $move.To
        if ($LASTEXITCODE -ne 0) {
            throw "git mv $($move.From) $($move.To) failed with exit $LASTEXITCODE"
        }
    }

    [PSCustomObject]@{
        Mode  = 'apply'
        Files = $plan.Edits.Count
        Moves = $plan.Moves.Count
    }
}
catch {
    Write-Error $_
    exit 1
}
