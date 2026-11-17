<#
.SYNOPSIS
    Порівняльне тестування Vortex Browser (WebView2) та Google Chrome на Windows 11.
.DESCRIPTION
    Запускає обидва браузери з однаковою кількістю вкладок (1, 5, 10, 20, 30),
    вимірює час запуску, сумарне споживання RAM (Working Set & Private Bytes усіх дочірніх процесів)
    та навантаження CPU, після чого формує JSON-звіт для імпорту в Vortex Browser (vortex://benchmarks).
#>

param(
    [string]$VortexExe = "$PSScriptRoot\..\src-tauri\target\release\vortex-browser.exe",
    [string]$ChromeExe = "C:\Program Files\Google\Chrome\Application\chrome.exe",
    [string]$OutputJson = "$PSScriptRoot\vortex_vs_chrome_report.json",
    [int[]]$TabCounts = @(1, 5, 10, 20, 30)
)

$TestUrls = @(
    "https://uk.wikipedia.org/wiki/Україна",
    "https://news.ycombinator.com",
    "https://example.com",
    "https://developer.mozilla.org",
    "https://www.rust-lang.org",
    "https://github.com/explore"
)

function Measure-BrowserScenario {
    param(
        [string]$BrowserName,
        [string]$ExecutablePath,
        [string]$ProcessPattern,
        [int]$TabCount,
        [bool]$IsChrome
    )

    if (-not (Test-Path $ExecutablePath)) {
        Write-Warning "Виконуваний файл $BrowserName не знайдено: $ExecutablePath"
        return $null
    }

    $UrlsToOpen = @()
    for ($i = 0; $i -lt $TabCount; $i++) {
        $UrlsToOpen += $TestUrls[$i % $TestUrls.Length]
    }

    $TempProfile = Join-Path $env:TEMP "vortex_bench_$($BrowserName)_$TabCount"
    if (Test-Path $TempProfile) { Remove-Item -Recurse -Force $TempProfile }
    New-Item -ItemType Directory -Path $TempProfile | Out-Null

    $Sw = [System.Diagnostics.Stopwatch]::StartNew()
    if ($IsChrome) {
        $Args = @("--user-data-dir=$TempProfile", "--no-first-run", "--no-default-browser-check") + $UrlsToOpen
        $Proc = Start-Process -FilePath $ExecutablePath -ArgumentList $Args -PassThru
    } else {
        $Args = @("--bench-tabs=$($UrlsToOpen -join ',')")
        $Proc = Start-Process -FilePath $ExecutablePath -ArgumentList $Args -PassThru
    }

    # Чекаємо стабілізації завантаження сторінок
    Start-Sleep -Seconds (6 + [Math]::Min($TabCount, 15))
    $Sw.Stop()

    $Procs = Get-Process | Where-Object { $_.ProcessName -match $ProcessPattern }
    $TotalWorkingSetMb = [Math]::Round((($Procs | Measure-Object -Property WorkingSet64 -Sum).Sum / 1MB), 1)
    $TotalPrivateMb = [Math]::Round((($Procs | Measure-Object -Property PrivateMemorySize64 -Sum).Sum / 1MB), 1)
    $CpuSeconds = [Math]::Round((($Procs | Measure-Object -Property CPU -Sum).Sum), 2)

    Stop-Process -Id $Proc.Id -Force -ErrorAction SilentlyContinue
    Get-Process | Where-Object { $_.ProcessName -match $ProcessPattern } | Stop-Process -Force -ErrorAction SilentlyContinue
    Start-Sleep -Seconds 2
    Remove-Item -Recurse -Force $TempProfile -ErrorAction SilentlyContinue

    return [PSCustomObject]@{
        browser         = $BrowserName
        tabs            = $TabCount
        startupAndLoadS = [Math]::Round($Sw.Elapsed.TotalSeconds, 2)
        workingSetMb    = $TotalWorkingSetMb
        privateBytesMb  = $TotalPrivateMb
        cpuSeconds      = $CpuSeconds
        processCount    = $Procs.Count
    }
}

$Results = @()
foreach ($Count in $TabCounts) {
    Write-Host "Тестування сценарію: $Count вкладок..." -ForegroundColor Cyan
    $vortexRes = Measure-BrowserScenario -BrowserName "Vortex Browser" -ExecutablePath $VortexExe -ProcessPattern "vortex-browser|msedgewebview2" -TabCount $Count -IsChrome $false
    if ($vortexRes) { $Results += $vortexRes }

    $chromeRes = Measure-BrowserScenario -BrowserName "Google Chrome" -ExecutablePath $ChromeExe -ProcessPattern "^chrome$" -TabCount $Count -IsChrome $true
    if ($chromeRes) { $Results += $chromeRes }
}

$Report = [PSCustomObject]@{
    generatedAt = (Get-Date).ToString("o")
    os          = (Get-CimInstance Win32_OperatingSystem).Caption
    cpu         = (Get-CimInstance Win32_Processor | Select-Object -First 1).Name
    results     = $Results
}

$Report | ConvertTo-Json -Depth 4 | Set-Content -Path $OutputJson -Encoding UTF8
Write-Host "Звіт збережено у: $OutputJson" -ForegroundColor Green
