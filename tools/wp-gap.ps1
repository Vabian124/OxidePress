#Requires -Version 7.0
<#
.SYNOPSIS
    Diff the pinned WordPress schema against OxidePress migrations.
.DESCRIPTION
    Downloads schema.php and version.php for a WordPress tag, parses CREATE TABLE
    statements, and writes gaps/schema.json. Also records the ALTER statements
    that CREATE TABLE IF NOT EXISTS will not apply to an older database.
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

$IndexLength = '191'
$RequiredTables = @('wp_posts', 'wp_options', 'wp_users')

function Get-NormalizedSql {
    param([string]$Text)
    $normalized = $Text.ToLowerInvariant()
    $normalized = $normalized.Replace('{$max_index_length}', $IndexLength)
    $normalized = $normalized.Replace('$max_index_length', $IndexLength)
    $normalized = [regex]::Replace($normalized, '\s+', ' ').Trim().TrimEnd(',')
    # schema.php quotes integer defaults. MySQL stores the same value either way.
    $normalized = [regex]::Replace($normalized, "default '(-?\d+)'", { param($match) "default $($match.Groups[1].Value)" })
    return $normalized
}

function ConvertTo-WpTableName {
    param([string]$Raw)
    $name = $Raw.Trim().Trim('`')
    $name = $name.TrimStart('{').TrimEnd('}')
    if ($name -match '^\$wpdb->(\w+)$') {
        return "wp_$($Matches[1])"
    }
    return $name
}

function Split-SqlList {
    param([string]$Body)
    $parts = [System.Collections.Generic.List[string]]::new()
    $depth = 0
    $start = 0
    for ($i = 0; $i -lt $Body.Length; $i++) {
        $char = $Body[$i]
        if ($char -eq '(') {
            $depth++
        }
        elseif ($char -eq ')') {
            $depth--
        }
        elseif ($char -eq ',' -and $depth -eq 0) {
            $part = $Body.Substring($start, $i - $start).Trim()
            if ($part.Length -gt 0) {
                $parts.Add($part)
            }
            $start = $i + 1
        }
    }
    $tail = $Body.Substring($start).Trim().TrimEnd(',')
    if ($tail.Length -gt 0) {
        $parts.Add($tail)
    }
    return $parts
}

function Get-BalancedBody {
    param(
        [string]$Sql,
        [int]$OpenIndex
    )
    $depth = 0
    for ($i = $OpenIndex; $i -lt $Sql.Length; $i++) {
        $char = $Sql[$i]
        if ($char -eq '(') {
            $depth++
        }
        elseif ($char -eq ')') {
            $depth--
            if ($depth -eq 0) {
                return [PSCustomObject]@{
                    Body  = $Sql.Substring($OpenIndex + 1, $i - $OpenIndex - 1)
                    End   = $i
                }
            }
        }
    }
    throw "Unbalanced parentheses in CREATE TABLE at offset $OpenIndex."
}

function ConvertTo-IndexRecord {
    param([string]$Normalized)
    $kind = 'key'
    $name = ''
    $columns = ''
    if ($Normalized -match '^primary key \((.*)\)$') {
        $kind = 'primary'
        $name = 'PRIMARY'
        $columns = ($Matches[1] -replace '\s', '')
    }
    elseif ($Normalized -match '^(unique key|unique index|fulltext key|fulltext index|spatial key|key|index) (\S+) \((.*)\)$') {
        $kind = $Matches[1]
        $name = $Matches[2]
        $columns = ($Matches[3] -replace '\s', '')
    }
    else {
        return $null
    }
    [PSCustomObject]@{
        Kind     = $kind
        Name     = $name
        Columns  = $columns
        Identity = "$kind $name ($columns)"
    }
}

function ConvertTo-ParsedTable {
    param(
        [string]$Sql,
        [System.Text.RegularExpressions.Match]$Match
    )
    $tableName = ConvertTo-WpTableName -Raw $Match.Groups[1].Value
    $openIndex = $Match.Index + $Match.Length - 1
    $balanced = Get-BalancedBody -Sql $Sql -OpenIndex $openIndex
    $columns = [ordered]@{}
    $indexes = [ordered]@{}
    foreach ($part in @(Split-SqlList -Body $balanced.Body)) {
        $normalized = Get-NormalizedSql -Text $part
        if ($normalized.Length -eq 0) {
            continue
        }
        $index = ConvertTo-IndexRecord -Normalized $normalized
        if ($null -ne $index) {
            $indexes[$index.Identity] = $index
            continue
        }
        $columnName = ($normalized -split ' ', 2)[0]
        $columns[$columnName] = $normalized
    }
    [PSCustomObject]@{
        Name    = $tableName
        Columns = $columns
        Indexes = $indexes
    }
}

function ConvertTo-TableMap {
    param([string]$Sql)
    # schema.php defines wp_users twice: single-site first, multisite second.
    # The first definition is what a single-site install creates.
    $tables = [ordered]@{}
    $variants = [System.Collections.Generic.List[object]]::new()
    $pattern = [regex]::new('CREATE TABLE(?:\s+IF NOT EXISTS)?\s+(\S+)\s*\(', [System.Text.RegularExpressions.RegexOptions]::IgnoreCase)
    foreach ($match in $pattern.Matches($Sql)) {
        $parsed = ConvertTo-ParsedTable -Sql $Sql -Match $match
        if (-not $tables.Contains($parsed.Name)) {
            $tables[$parsed.Name] = $parsed
            continue
        }
        $first = $tables[$parsed.Name]
        $extraColumns = @($parsed.Columns.Keys | Where-Object { -not $first.Columns.Contains($_) } | Sort-Object)
        $variants.Add([ordered]@{
                table          = $parsed.Name
                extra_columns  = $extraColumns
            })
    }
    [PSCustomObject]@{
        Tables   = $tables
        Variants = $variants
    }
}

function Get-PinnedFile {
    param(
        [string]$Url,
        [string]$Destination,
        [bool]$Force
    )
    if ((Test-Path -LiteralPath $Destination) -and -not $Force) {
        return
    }
    $directory = Split-Path -Parent $Destination
    if (-not (Test-Path -LiteralPath $directory)) {
        New-Item -ItemType Directory -Path $directory | Out-Null
    }
    Invoke-WebRequest -Uri $Url -OutFile $Destination -Headers @{ 'User-Agent' = 'OxidePress-wp-gap' } -UseBasicParsing
}

function Get-RustSchemaSql {
    param([string]$MigrateFile)
    $text = [System.IO.File]::ReadAllText($MigrateFile)
    $parts = [regex]::Matches($text, '(?s)"(CREATE TABLE IF NOT EXISTS .*?)"')
    if ($parts.Count -eq 0) {
        throw "No CREATE TABLE strings found in $MigrateFile."
    }
    return (($parts | ForEach-Object { $_.Groups[1].Value }) -join "`n")
}

function Get-DbVersionFromRust {
    param([string]$MigrateFile)
    $text = [System.IO.File]::ReadAllText($MigrateFile)
    $match = [regex]::Match($text, '\("db_version",\s*"(\d+)"\)')
    if (-not $match.Success) {
        throw "db_version stamp not found in $MigrateFile."
    }
    return [int]$match.Groups[1].Value
}

function Get-WordPressVersions {
    param([string]$VersionFile)
    $text = [System.IO.File]::ReadAllText($VersionFile)
    $version = [regex]::Match($text, '\$wp_version\s*=\s*''([^'']+)''')
    $dbVersion = [regex]::Match($text, '\$wp_db_version\s*=\s*(\d+)')
    if (-not $version.Success -or -not $dbVersion.Success) {
        throw "Could not read wp_version or wp_db_version from $VersionFile."
    }
    [PSCustomObject]@{
        Version   = $version.Groups[1].Value
        DbVersion = [int]$dbVersion.Groups[1].Value
    }
}

function Get-ExistingDatabaseRepairs {
    param($Tables)
    $posts = $Tables['wp_posts']
    $authorIndex = @($posts.Indexes.Values | Where-Object { $_.Name -eq 'type_status_author' })
    if ($authorIndex.Count -ne 1) {
        throw 'Pinned schema is missing the wp_posts type_status_author index.'
    }
    $comments = $Tables['wp_comments']
    if (-not $comments.Columns.Contains('comment_author')) {
        throw 'Pinned schema is missing wp_comments.comment_author.'
    }
    $column = [string]$comments.Columns['comment_author']
    @(
        [ordered]@{
            id    = 'wp_posts.type_status_author'
            kind  = 'add_index'
            table = 'wp_posts'
            name  = 'type_status_author'
            sql   = "ALTER TABLE wp_posts ADD KEY type_status_author ($($authorIndex[0].Columns))"
        },
        [ordered]@{
            id    = 'wp_comments.comment_author'
            kind  = 'modify_column'
            table = 'wp_comments'
            name  = 'comment_author'
            sql   = "ALTER TABLE wp_comments MODIFY $column"
        }
    )
}

function Assert-RequiredTables {
    param(
        $Tables,
        [string]$Label
    )
    foreach ($name in $RequiredTables) {
        if (-not $Tables.Contains($name)) {
            throw "$Label schema parse missed $name."
        }
    }
}

try {
    $root = Split-Path -Parent $PSScriptRoot
    $cache = Join-Path $root "tools/wp-ref/$Tag"
    $schemaPath = Join-Path $cache 'schema.php'
    $versionPath = Join-Path $cache 'version.php'
    $migrateFile = Join-Path $root 'crates/oxidepress-migrate/src/lib.rs'
    $reportPath = Join-Path $root 'gaps/schema.json'

    $base = "https://raw.githubusercontent.com/WordPress/WordPress/$Tag"
    Get-PinnedFile -Url "$base/wp-admin/includes/schema.php" -Destination $schemaPath -Force ([bool]$Refresh)
    Get-PinnedFile -Url "$base/wp-includes/version.php" -Destination $versionPath -Force ([bool]$Refresh)

    $wordpressMeta = Get-WordPressVersions -VersionFile $versionPath
    if ($wordpressMeta.Version -ne $Tag) {
        throw "Pinned tag $Tag but version.php says $($wordpressMeta.Version)."
    }

    $wordpressParsed = ConvertTo-TableMap -Sql ([System.IO.File]::ReadAllText($schemaPath))
    $oxideSql = Get-RustSchemaSql -MigrateFile $migrateFile
    $oxideParsed = ConvertTo-TableMap -Sql $oxideSql
    $wordpressTables = $wordpressParsed.Tables
    $oxideTables = $oxideParsed.Tables
    Assert-RequiredTables -Tables $wordpressTables -Label 'WordPress'
    Assert-RequiredTables -Tables $oxideTables -Label 'OxidePress'
    $oxideDbVersion = Get-DbVersionFromRust -MigrateFile $migrateFile

    $multisiteNames = [System.Collections.Generic.HashSet[string]]::new(
        [string[]]@(
            'wp_blogs',
            'wp_blogmeta',
            'wp_registration_log',
            'wp_signups',
            'wp_site',
            'wp_sitemeta',
            'wp_sitecategories'
        )
    )
    $absent = @($wordpressTables.Keys | Where-Object { -not $oxideTables.Contains($_) } | Sort-Object)
    $missingTables = @($absent | Where-Object { -not $multisiteNames.Contains($_) })
    $multisiteAbsent = @($absent | Where-Object { $multisiteNames.Contains($_) })
    $extraTables = @($oxideTables.Keys | Where-Object { -not $wordpressTables.Contains($_) } | Sort-Object)
    $clean = [System.Collections.Generic.List[string]]::new()
    $diffs = [System.Collections.Generic.List[object]]::new()

    foreach ($name in ($oxideTables.Keys | Where-Object { $wordpressTables.Contains($_) } | Sort-Object)) {
        $wordPress = $wordpressTables[$name]
        $oxide = $oxideTables[$name]
        $missingColumns = @($wordPress.Columns.Keys | Where-Object { -not $oxide.Columns.Contains($_) } | Sort-Object)
        $extraColumns = @($oxide.Columns.Keys | Where-Object { -not $wordPress.Columns.Contains($_) } | Sort-Object)
        $columnMismatches = [System.Collections.Generic.List[object]]::new()
        foreach ($column in ($wordPress.Columns.Keys | Where-Object { $oxide.Columns.Contains($_) })) {
            if ($wordPress.Columns[$column] -ne $oxide.Columns[$column]) {
                $columnMismatches.Add([ordered]@{
                        column     = $column
                        wordpress  = $wordPress.Columns[$column]
                        oxidepress = $oxide.Columns[$column]
                    })
            }
        }
        $missingIndexes = @($wordPress.Indexes.Keys | Where-Object { -not $oxide.Indexes.Contains($_) } | Sort-Object)
        $extraIndexes = @($oxide.Indexes.Keys | Where-Object { -not $wordPress.Indexes.Contains($_) } | Sort-Object)
        $hasDiff = $missingColumns.Count -gt 0 -or $extraColumns.Count -gt 0 -or $columnMismatches.Count -gt 0 -or $missingIndexes.Count -gt 0 -or $extraIndexes.Count -gt 0
        if (-not $hasDiff) {
            $clean.Add($name)
            continue
        }
        $diffs.Add([ordered]@{
                table              = $name
                missing_columns    = $missingColumns
                extra_columns      = $extraColumns
                column_mismatches  = @($columnMismatches)
                missing_indexes    = $missingIndexes
                extra_indexes      = $extraIndexes
            })
    }

    $repairs = @(Get-ExistingDatabaseRepairs -Tables $wordpressTables)

    $report = [ordered]@{
        pin                     = $Tag
        wordpress_version       = $wordpressMeta.Version
        wordpress_db_version    = $wordpressMeta.DbVersion
        oxidepress_db_version   = $oxideDbVersion
        coverage                = 'Single-site CREATE TABLE statements. Integer defaults are compared numerically. The second wp_users definition is the multisite variant and is not required for a single-site install. existing_database_repairs are ALTER statements for databases created before this pin; CREATE TABLE IF NOT EXISTS does not apply them. SeaORM entities and PHP behavior are outside this report.'
        sources                 = [ordered]@{
            wordpress_schema = "$base/wp-admin/includes/schema.php"
            wordpress_version = "$base/wp-includes/version.php"
            oxidepress        = 'crates/oxidepress-migrate/src/lib.rs'
        }
        missing_tables          = $missingTables
        multisite_tables_not_in_single_site_migrator = $multisiteAbsent
        multisite_variants      = @($wordpressParsed.Variants)
        extra_tables            = $extraTables
        clean_tables            = @($clean)
        tables                  = @($diffs)
        existing_database_repairs = $repairs
    }

    $gapsDir = Split-Path -Parent $reportPath
    if (-not (Test-Path -LiteralPath $gapsDir)) {
        New-Item -ItemType Directory -Path $gapsDir | Out-Null
    }
    $json = $report | ConvertTo-Json -Depth 8
    $utf8 = [System.Text.UTF8Encoding]::new($false)
    [System.IO.File]::WriteAllText($reportPath, $json + "`n", $utf8)

    Write-Host "pin: $Tag db $($wordpressMeta.DbVersion)"
    Write-Host "oxidepress db_version: $oxideDbVersion"
    Write-Host "missing single-site tables: $($missingTables.Count)"
    Write-Host "multisite tables left to a later migrator: $($multisiteAbsent.Count)"
    Write-Host "tables with column or index drift: $($diffs.Count)"
    Write-Host "clean tables: $($clean.Count)"
    Write-Host "existing-database repairs: $($repairs.Count)"
    Write-Host "wrote gaps/schema.json"

    $gapCount = $missingTables.Count + $extraTables.Count + $diffs.Count
    if ($wordpressMeta.DbVersion -ne $oxideDbVersion) {
        $gapCount++
    }
    if ($FailOnGap -and $gapCount -gt 0) {
        exit 2
    }

    [PSCustomObject]@{
        Pin            = $Tag
        WordPressDb    = $wordpressMeta.DbVersion
        OxidePressDb   = $oxideDbVersion
        MissingTables  = $missingTables.Count
        MultisiteTables = $multisiteAbsent.Count
        DriftedTables  = $diffs.Count
        CleanTables    = $clean.Count
    }
}
catch {
    Write-Error $_
    exit 1
}
