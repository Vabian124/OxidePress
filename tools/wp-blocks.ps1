#Requires -Version 7.0
<#
.SYNOPSIS
    Diff pinned WordPress core blocks against OxidePress.
.DESCRIPTION
    Reads block.json files for a WordPress tag and writes gaps/blocks.json.
    Does not modify Rust sources.
#>
[CmdletBinding()]
param(
    [string]$Tag = '7.1.4',
    [switch]$Refresh,
    [switch]$FailOnGap
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

function Get-RegisteredBlockNames {
    param([string]$Path)
    $text = [System.IO.File]::ReadAllText($Path)
    $names = [System.Collections.Generic.HashSet[string]]::new()
    foreach ($match in [regex]::Matches($text, 'core/[a-z0-9-]+')) {
        [void]$names.Add($match.Value)
    }
    return $names
}

function Get-BlockDirectories {
    param([string]$Tag)
    $uri = "https://api.github.com/repos/WordPress/WordPress/contents/wp-includes/blocks?ref=$Tag"
    $items = Invoke-RestMethod -Uri $uri -Headers @{
        'User-Agent' = 'OxidePress-wp-blocks'
        'Accept'     = 'application/vnd.github+json'
    }
    @($items | Where-Object { $_.type -eq 'dir' } | Select-Object -ExpandProperty name | Sort-Object)
}

try {
    $root = Split-Path -Parent $PSScriptRoot
    $cacheRoot = Join-Path $root "tools/wp-ref/$Tag/blocks"
    if (-not (Test-Path -LiteralPath $cacheRoot)) {
        New-Item -ItemType Directory -Path $cacheRoot | Out-Null
    }

    $directories = @(Get-BlockDirectories -Tag $Tag)
    if ($directories.Count -lt 20) {
        throw "Block directory listing for $Tag returned $($directories.Count) entries."
    }

    $blocks = [System.Collections.Generic.List[object]]::new()
    foreach ($directory in $directories) {
        $destination = Join-Path $cacheRoot "$directory/block.json"
        if ($Refresh -or -not (Test-Path -LiteralPath $destination)) {
            $parent = Split-Path -Parent $destination
            if (-not (Test-Path -LiteralPath $parent)) {
                New-Item -ItemType Directory -Path $parent | Out-Null
            }
            $url = "https://raw.githubusercontent.com/WordPress/WordPress/$Tag/wp-includes/blocks/$directory/block.json"
            try {
                Invoke-WebRequest -Uri $url -OutFile $destination -Headers @{ 'User-Agent' = 'OxidePress-wp-blocks' } -UseBasicParsing
            }
            catch {
                Write-Warning "No block.json for $directory"
                continue
            }
        }
        $json = Get-Content -LiteralPath $destination -Raw | ConvertFrom-Json
        $name = ''
        if ($json.PSObject.Properties.Name -contains 'name') {
            $name = [string]$json.name
        }
        if ([string]::IsNullOrWhiteSpace($name)) {
            $name = "core/$directory"
        }
        $title = ''
        if ($json.PSObject.Properties.Name -contains 'title') {
            $title = [string]$json.title
        }
        $category = ''
        if ($json.PSObject.Properties.Name -contains 'category') {
            $category = [string]$json.category
        }
        $description = ''
        if ($json.PSObject.Properties.Name -contains 'description') {
            $description = [string]$json.description
        }
        $blocks.Add([ordered]@{
                name        = $name
                title       = $title
                category    = $category
                description = $description
            })
    }

    $rendererNames = [System.Collections.Generic.HashSet[string]]::new()
    foreach ($relative in @(
            'crates/oxidepress-blocks/src/core_blocks.rs',
            'crates/oxidepress-blocks/src/pinned.rs',
            'crates/oxidepress-blocks/src/renderer.rs'
        )) {
        foreach ($name in @(Get-RegisteredBlockNames -Path (Join-Path $root $relative))) {
            [void]$rendererNames.Add($name)
        }
    }
    $restNames = Get-RegisteredBlockNames -Path (Join-Path $root 'crates/oxidepress-api/src/block_types.rs')
    $wordpressNames = [System.Collections.Generic.HashSet[string]]::new()
    foreach ($block in $blocks) {
        [void]$wordpressNames.Add([string]$block.name)
    }

    $missingRenderer = @($wordpressNames | Where-Object { -not $rendererNames.Contains($_) } | Sort-Object)
    $missingRest = @($wordpressNames | Where-Object { -not $restNames.Contains($_) } | Sort-Object)
    $rendererOnly = @($rendererNames | Where-Object { -not $wordpressNames.Contains($_) } | Sort-Object)
    $restOnly = @($restNames | Where-Object { -not $wordpressNames.Contains($_) } | Sort-Object)

    $report = [ordered]@{
        pin                    = $Tag
        coverage               = 'Core block.json names under wp-includes/blocks. Renderer names are string literals in oxidepress-blocks. REST names are string literals in oxidepress-api block_types.rs. Aliases such as core/query-loop may appear only on the OxidePress side.'
        wordpress_count        = $blocks.Count
        wordpress_blocks       = @($blocks | Sort-Object { $_.name })
        missing_from_renderer  = $missingRenderer
        missing_from_rest      = $missingRest
        renderer_only          = $rendererOnly
        rest_only              = $restOnly
    }

    $reportPath = Join-Path $root 'gaps/blocks.json'
    $jsonText = $report | ConvertTo-Json -Depth 6
    $utf8 = [System.Text.UTF8Encoding]::new($false)
    [System.IO.File]::WriteAllText($reportPath, $jsonText + "`n", $utf8)

    Write-Host "pin: $Tag"
    Write-Host "wordpress core blocks: $($blocks.Count)"
    Write-Host "missing from renderer: $($missingRenderer.Count)"
    Write-Host "missing from REST catalog: $($missingRest.Count)"
    Write-Host "wrote gaps/blocks.json"

    if ($FailOnGap -and (($missingRenderer.Count + $missingRest.Count) -gt 0)) {
        exit 2
    }

    [PSCustomObject]@{
        Pin              = $Tag
        WordPressBlocks  = $blocks.Count
        MissingRenderer  = $missingRenderer.Count
        MissingRest      = $missingRest.Count
    }
}
catch {
    Write-Error $_
    exit 1
}
