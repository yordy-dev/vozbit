$ErrorActionPreference = 'Stop'
[Console]::InputEncoding = [Text.UTF8Encoding]::new($false)
[Console]::OutputEncoding = [Text.UTF8Encoding]::new($false)
. (Join-Path $PSScriptRoot 'correction-diff.ps1')

function Stop-Observation([string]$reason) {
    if ($env:VOZBIT_LEARNING_TEST -eq '1') { [Console]::Error.WriteLine($reason) }
    exit
}

function Get-FieldPatterns($element) {
    $textPattern = $null
    $valuePattern = $null
    $legacyPattern = $null
    if (-not $element.TryGetCurrentPattern([System.Windows.Automation.TextPattern]::Pattern, [ref]$textPattern)) {
        if ($element.TryGetCurrentPattern([System.Windows.Automation.ValuePattern]::Pattern, [ref]$valuePattern)) {
            if ($valuePattern.Current.IsReadOnly) { return $null }
        } else {
            $handle = [IntPtr]$element.Current.NativeWindowHandle
            if ($handle -eq [IntPtr]::Zero) { return $null }
            $legacyPattern = $handle
        }
    }
    return [pscustomobject]@{ Text = $textPattern; Value = $valuePattern; Legacy = $legacyPattern }
}

function Read-FieldText($patterns) {
    if ($null -ne $patterns.Text) { return [string]$patterns.Text.DocumentRange.GetText(40001) }
    if ($null -ne $patterns.Value) { return [string]$patterns.Value.Current.Value }
    return [VozbitLearningWindowText]::Read($patterns.Legacy)
}

function Get-CaretOffset($patterns) {
    if ($null -eq $patterns.Text) { return $null }
    $ranges = $patterns.Text.GetSelection()
    if ($ranges.Length -ne 1) { return $null }
    $before = $patterns.Text.DocumentRange.Clone()
    $before.MoveEndpointByRange(
        [System.Windows.Automation.Text.TextPatternRangeEndpoint]::End,
        $ranges[0],
        [System.Windows.Automation.Text.TextPatternRangeEndpoint]::Start)
    return $before.GetText(40001).Length
}

function Get-ElementIdentity($element) {
    $rectangle = $element.Current.BoundingRectangle
    return [pscustomobject]@{
        Runtime = ($element.GetRuntimeId() -join ',')
        AutomationId = [string]$element.Current.AutomationId
        ControlType = [string]$element.Current.ControlType.ProgrammaticName
        Left = [double]$rectangle.Left
        Top = [double]$rectangle.Top
        Width = [double]$rectangle.Width
        Height = [double]$rectangle.Height
    }
}

function Test-SameEditable($expected, $current) {
    if ($current.Runtime -ceq $expected.Runtime) { return $true }
    if ($current.ControlType -ne $expected.ControlType) { return $false }
    if ($expected.AutomationId.Length -gt 0 -and $current.AutomationId -ceq $expected.AutomationId) { return $true }
    $centerX1 = $expected.Left + ($expected.Width / 2)
    $centerY1 = $expected.Top + ($expected.Height / 2)
    $centerX2 = $current.Left + ($current.Width / 2)
    $centerY2 = $current.Top + ($current.Height / 2)
    return ([Math]::Abs($centerX1 - $centerX2) -le 16 -and [Math]::Abs($centerY1 - $centerY2) -le 16)
}

function ConvertTo-NormalizedText([string]$text) {
    $builder = [Text.StringBuilder]::new()
    $map = [System.Collections.Generic.List[int]]::new()
    $inWhitespace = $false
    for ($index = 0; $index -lt $text.Length; $index++) {
        $character = $text[$index]
        if ([char]::IsWhiteSpace($character) -or $character -eq [char]0x00A0) {
            if (-not $inWhitespace) {
                [void]$builder.Append(' ')
                $map.Add($index)
                $inWhitespace = $true
            }
        } else {
            [void]$builder.Append($character)
            $map.Add($index)
            $inWhitespace = $false
        }
    }
    return [pscustomobject]@{ Text = $builder.ToString(); Map = $map; OriginalLength = $text.Length }
}

function Find-DictatedSpan([string]$baseline, [string]$dictated, $caretOffset) {
    $variants = @($dictated)
    if ($dictated.EndsWith(' ')) { $variants += $dictated.TrimEnd() }
    foreach ($variant in $variants | Select-Object -Unique) {
        if ($variant.Length -eq 0) { continue }
        $positions = [System.Collections.Generic.List[int]]::new()
        $position = $baseline.IndexOf($variant, [StringComparison]::Ordinal)
        while ($position -ge 0) {
            $positions.Add($position)
            $position = $baseline.IndexOf($variant, $position + 1, [StringComparison]::Ordinal)
        }
        if ($positions.Count -gt 0) {
            $chosen = $positions[$positions.Count - 1]
            if ($null -ne $caretOffset) {
                $eligible = @($positions | Where-Object { $_ + $variant.Length -le $caretOffset + 1 })
                if ($eligible.Count -gt 0) { $chosen = $eligible[$eligible.Count - 1] }
            }
            return [pscustomobject]@{ Start = $chosen; Length = $variant.Length }
        }
    }

    $normalizedBaseline = ConvertTo-NormalizedText $baseline
    $normalizedDictated = (ConvertTo-NormalizedText $dictated.TrimEnd()).Text.Trim()
    if ($normalizedDictated.Length -eq 0) { return $null }
    $normalizedStart = $normalizedBaseline.Text.LastIndexOf($normalizedDictated, [StringComparison]::Ordinal)
    if ($normalizedStart -lt 0) { return $null }
    $start = $normalizedBaseline.Map[$normalizedStart]
    $lastNormalized = $normalizedStart + $normalizedDictated.Length - 1
    $end = if ($lastNormalized + 1 -lt $normalizedBaseline.Map.Count) {
        $normalizedBaseline.Map[$lastNormalized + 1]
    } else { $normalizedBaseline.OriginalLength }
    return [pscustomobject]@{ Start = $start; Length = $end - $start }
}

function Find-TargetElement([int]$targetPid, [string]$dictated) {
    $script:targetDebug = [System.Collections.Generic.List[string]]::new()
    $elements = [System.Collections.Generic.List[object]]::new()
    $focused = [System.Windows.Automation.AutomationElement]::FocusedElement
    if ($null -ne $focused -and $focused.Current.ProcessId -eq $targetPid) { $elements.Add($focused) }
    $condition = [System.Windows.Automation.PropertyCondition]::new(
        [System.Windows.Automation.AutomationElement]::ProcessIdProperty,
        $targetPid)
    foreach ($candidate in [System.Windows.Automation.AutomationElement]::RootElement.FindAll(
        [System.Windows.Automation.TreeScope]::Descendants,
        $condition)) {
        $elements.Add($candidate)
    }
    $best = $null
    foreach ($candidate in $elements) {
        try {
            if ($candidate.Current.IsPassword) { continue }
            $candidatePatterns = Get-FieldPatterns $candidate
            if ($null -eq $candidatePatterns) { continue }
            $candidateText = Read-FieldText $candidatePatterns
            if ($env:VOZBIT_LEARNING_TEST -eq '1') {
                $script:targetDebug.Add("$($candidate.Current.ControlType.ProgrammaticName):$($candidateText.Length):$($candidateText | ConvertTo-Json -Compress)")
            }
            if ($candidateText.Length -gt 40000) { continue }
            $candidateSpan = @(Find-DictatedSpan $candidateText $dictated $null) |
                Where-Object { $null -ne $_ } |
                Select-Object -Last 1
            if ($env:VOZBIT_LEARNING_TEST -eq '1') { $script:targetDebug.Add("span:$($candidateSpan.Start):$($candidateSpan.Length)") }
            if ($null -eq $candidateSpan) { continue }
            if ($null -ne $best -and $best.Baseline.Length -le $candidateText.Length) { continue }
            $best = [pscustomobject]@{
                Element = $candidate
                Patterns = $candidatePatterns
                Baseline = $candidateText
                Span = $candidateSpan
            }
            if ($env:VOZBIT_LEARNING_TEST -eq '1') { $script:targetDebug.Add('best-set') }
        } catch {
            if ($env:VOZBIT_LEARNING_TEST -eq '1') { $script:targetDebug.Add("error:$($_.Exception.Message)") }
            continue
        }
    }
    return $best
}

function Write-Candidates([string]$before, [string]$after) {
    [object[]]$candidates = @(Get-CorrectionCandidates $before $after)
    if ($candidates.Count -eq 0) { return 0 }
    $batch = [ordered]@{
        original = $before
        edited = $after
        candidates = $candidates
    }
    [Console]::Out.WriteLine(($batch | ConvertTo-Json -Compress -Depth 5))
    [Console]::Out.Flush()
    return $candidates.Count
}

try {
    Add-Type -AssemblyName UIAutomationClient
    Add-Type -AssemblyName UIAutomationTypes
    Add-Type @'
using System;
using System.Runtime.InteropServices;
using System.Text;
public static class VozbitLearningWindowText {
    [DllImport("user32.dll", CharSet = CharSet.Unicode)] static extern int GetWindowTextLength(IntPtr hWnd);
    [DllImport("user32.dll", CharSet = CharSet.Unicode)] static extern int GetWindowText(IntPtr hWnd, StringBuilder value, int maxCount);
    public static string Read(IntPtr hWnd) {
        var length = GetWindowTextLength(hWnd);
        var value = new StringBuilder(Math.Max(1, length + 1));
        GetWindowText(hWnd, value, value.Capacity);
        return value.ToString();
    }
}
'@
    $request = [Console]::ReadLine() | ConvertFrom-Json
    $element = [System.Windows.Automation.AutomationElement]::FocusedElement
    if ($request.selection) {
        if ($null -eq $element -or $element.Current.IsPassword -or $element.Current.ProcessId -ne $request.pid) { Stop-Observation 'initial-focus' }
        $patterns = Get-FieldPatterns $element
        if ($null -eq $patterns) { Stop-Observation 'initial-pattern' }
        if ($null -eq $patterns.Text) { exit }
        $ranges = $patterns.Text.GetSelection()
        if ($ranges.Length -ne 1) { exit }
        $selected = $ranges[0].GetText(81).Trim()
        if ($selected.Length -lt 1 -or $selected.Length -gt 80 -or $selected -match '[\r\n]') { exit }
        @{term=$selected;replacement=$selected} | ConvertTo-Json -Compress
        exit
    }

    $target = Find-TargetElement ([int]$request.pid) ([string]$request.text)
    if ($null -eq $target) {
        if ($env:VOZBIT_LEARNING_TEST -eq '1') { Stop-Observation ("initial-target " + ($script:targetDebug -join '|')) }
        Stop-Observation 'initial-target'
    }
    $element = $target.Element
    $patterns = $target.Patterns
    $baseline = $target.Baseline
    $span = $target.Span
    $identity = Get-ElementIdentity $element
    if ($null -eq $span) {
        if ($env:VOZBIT_LEARNING_TEST -eq '1') {
            Stop-Observation ("dictated-span baseline=" + ($baseline | ConvertTo-Json -Compress) + " request=" + ([string]$request.text | ConvertTo-Json -Compress))
        }
        Stop-Observation 'dictated-span'
    }
    $prefix = $baseline.Substring(0, $span.Start)
    $dictated = $baseline.Substring($span.Start, $span.Length)
    $suffix = $baseline.Substring($span.Start + $span.Length)
    if ($env:VOZBIT_LEARNING_TEST -eq '1') { [Console]::Error.WriteLine('ready') }
    $last = ''
    $stable = 0
    $focusMisses = 0
    for ($tick = 0; $tick -lt 200; $tick++) {
        Start-Sleep -Milliseconds 150
        if ($null -eq (Get-Process -Id $request.owner -ErrorAction SilentlyContinue)) { Stop-Observation 'owner-ended' }
        $currentElement = [System.Windows.Automation.AutomationElement]::FocusedElement
        if ($null -ne $currentElement -and $currentElement.Current.ProcessId -eq $request.pid -and $currentElement.Current.IsPassword) {
            Stop-Observation 'password-focus'
        }
        $observedElement = $null
        $currentIdentity = $null
        if ($null -ne $currentElement -and $currentElement.Current.ProcessId -eq $request.pid) {
            $currentIdentity = Get-ElementIdentity $currentElement
            if (Test-SameEditable $identity $currentIdentity) { $observedElement = $currentElement }
        }
        if ($null -eq $observedElement) {
            try {
                if ($element.Current.ProcessId -eq $request.pid -and -not $element.Current.IsPassword) {
                    $originalIdentity = Get-ElementIdentity $element
                    if (Test-SameEditable $identity $originalIdentity) {
                        $observedElement = $element
                        $currentIdentity = $originalIdentity
                    }
                }
            } catch { $observedElement = $null }
        }
        if ($null -eq $observedElement) {
            $focusMisses++
            if ($focusMisses -ge 10) { Stop-Observation 'unavailable-editable' }
            continue
        }
        $focusMisses = 0
        $currentPatterns = Get-FieldPatterns $observedElement
        if ($null -eq $currentPatterns) { Stop-Observation 'changed-pattern' }
        $current = Read-FieldText $currentPatterns
        if ($current.Length -gt 40000 -or -not $current.StartsWith($prefix, [StringComparison]::Ordinal) -or -not $current.EndsWith($suffix, [StringComparison]::Ordinal)) {
            if ($env:VOZBIT_LEARNING_TEST -eq '1') {
                Stop-Observation ("changed-anchors current=" + ($current | ConvertTo-Json -Compress) + " prefix=" + ($prefix | ConvertTo-Json -Compress) + " suffix=" + ($suffix | ConvertTo-Json -Compress))
            }
            Stop-Observation 'changed-anchors'
        }
        $length = $current.Length - $prefix.Length - $suffix.Length
        if ($length -lt 0) { exit }
        $changed = $current.Substring($prefix.Length, $length)
        if ($changed -ceq $dictated) { $last = ''; $stable = 0; continue }
        if ($env:VOZBIT_LEARNING_TEST -eq '1' -and $stable -eq 0) { [Console]::Error.WriteLine('change-seen') }
        if ($changed -ceq $last) { $stable++ } else { $last = $changed; $stable = 0 }
        # About 450 ms avoids learning half-typed words without requiring the
        # user to wait several seconds before sending a long paragraph.
        if ($stable -lt 3) { continue }
        $written = Write-Candidates $dictated $changed
        if ($env:VOZBIT_LEARNING_TEST -eq '1') { [Console]::Error.WriteLine("candidates=$written") }
        $dictated = $changed
        $last = ''
        $stable = 0
        $identity = $currentIdentity
        $element = $observedElement
    }
} catch {
    if ($env:VOZBIT_LEARNING_TEST -eq '1') { [Console]::Error.WriteLine("exception: $($_.Exception.Message)") }
    exit
}
