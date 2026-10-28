$ErrorActionPreference = 'Stop'
[Console]::InputEncoding = [Text.UTF8Encoding]::new($false)
[Console]::OutputEncoding = [Text.UTF8Encoding]::new($false)
. (Join-Path $PSScriptRoot 'correction-diff.ps1')
function Read-FieldText($textPattern, $valuePattern) {
    if ($null -ne $textPattern) { return [string]$textPattern.DocumentRange.GetText(20001) }
    return [string]$valuePattern.Current.Value
}
try {
    Add-Type -AssemblyName UIAutomationClient
    Add-Type -AssemblyName UIAutomationTypes
    $request = [Console]::ReadLine() | ConvertFrom-Json
    $element = [System.Windows.Automation.AutomationElement]::FocusedElement
    if ($null -eq $element -or $element.Current.IsPassword -or $element.Current.ProcessId -ne $request.pid) { exit }
    $identity = $element.GetRuntimeId() -join ','
    $pattern = $null; $valuePattern = $null
    if (-not $element.TryGetCurrentPattern([System.Windows.Automation.TextPattern]::Pattern, [ref]$pattern)) {
        if (-not $element.TryGetCurrentPattern([System.Windows.Automation.ValuePattern]::Pattern, [ref]$valuePattern)) { exit }
        if ($valuePattern.Current.IsReadOnly) { exit }
    }
    if ($request.selection) {
        if ($null -eq $pattern) { exit }
        $ranges=$pattern.GetSelection()
        if ($ranges.Length -ne 1) { exit }
        $selected=$ranges[0].GetText(81).Trim()
        if ($selected.Length -lt 1 -or $selected.Length -gt 80 -or $selected -match '[\r\n]') { exit }
        @{term=$selected;replacement=$selected} | ConvertTo-Json -Compress
        exit
    }
    $baseline = Read-FieldText $pattern $valuePattern
    if ($baseline.Length -gt 20000) { exit }
    $dictated = [string]$request.text
    $start = $baseline.IndexOf($dictated, [StringComparison]::Ordinal)
    if ($start -lt 0 -and $dictated.EndsWith(' ')) {
        $withoutSpace = $dictated.TrimEnd(' ')
        $start = $baseline.IndexOf($withoutSpace, [StringComparison]::Ordinal)
        if ($start -ge 0) { $dictated = $withoutSpace }
    }
    if ($start -lt 0 -or $baseline.IndexOf($dictated, $start + $dictated.Length, [StringComparison]::Ordinal) -ge 0) { exit }
    $prefix = $baseline.Substring(0,$start)
    $suffix = $baseline.Substring($start+$dictated.Length)
    $last = ''; $stable = 0
    for ($tick=0; $tick -lt 100; $tick++) {
        Start-Sleep -Milliseconds 300
        if ($null -eq (Get-Process -Id $request.owner -ErrorAction SilentlyContinue)) { exit }
        $currentElement = [System.Windows.Automation.AutomationElement]::FocusedElement
        if ($null -eq $currentElement -or ($currentElement.GetRuntimeId() -join ',') -ne $identity -or $currentElement.Current.IsPassword) { exit }
        $current = Read-FieldText $pattern $valuePattern
        if ($current.Length -gt 20000 -or -not $current.StartsWith($prefix,[StringComparison]::Ordinal) -or -not $current.EndsWith($suffix,[StringComparison]::Ordinal)) { exit }
        $length = $current.Length-$prefix.Length-$suffix.Length
        if ($length -lt 0) { exit }
        $changed = $current.Substring($prefix.Length,$length)
        if ($changed -ceq $dictated) { $last='';$stable=0;continue }
        if ($changed -ceq $last) { $stable++ } else { $last=$changed;$stable=0 }
        # Wait for the edit to settle before treating a partially typed word as final.
        if ($stable -lt 8) { continue }
        $candidate = Get-CorrectionCandidate $dictated $changed
        if ($null -eq $candidate) { exit }
        $candidate | ConvertTo-Json -Compress
        [Console]::Out.Flush()
        $dictated = $changed
        $last = ''; $stable = 0
    }
} catch { exit }
