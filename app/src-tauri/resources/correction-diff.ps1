function Get-CorrectionTokens([string]$text) {
    return @([regex]::Matches($text, '[\p{L}\p{M}\p{N}_]+|[^\p{L}\p{M}\p{N}_\s]+') | ForEach-Object {
        [pscustomobject]@{ Value = $_.Value; Index = $_.Index; Length = $_.Length }
    })
}

function Get-TokenSlice([string]$text, $tokens, [int]$start, [int]$end) {
    if ($start -ge $end -or $start -lt 0 -or $end -gt $tokens.Count) { return '' }
    $from = [int]$tokens[$start].Index
    $last = $tokens[$end - 1]
    return $text.Substring($from, ([int]$last.Index + [int]$last.Length) - $from).Trim()
}

function Test-SafeCorrectionValue([string]$value) {
    if ([string]::IsNullOrWhiteSpace($value) -or $value.Length -gt 80) { return $false }
    if ($value -match '[\r\n@:\\]' -or $value -match '\d{6,}') { return $false }
    return (($value -split '\s+').Count -le 7)
}

function New-CorrectionCandidate(
    [string]$dictated,
    [string]$changed,
    $oldTokens,
    $newTokens,
    [int]$oldStart,
    [int]$oldEnd,
    [int]$newStart,
    [int]$newEnd
) {
    $old = Get-TokenSlice $dictated $oldTokens $oldStart $oldEnd
    $new = Get-TokenSlice $changed $newTokens $newStart $newEnd
    $needsContext = [string]::IsNullOrWhiteSpace($old) -or [string]::IsNullOrWhiteSpace($new)
    $singleWords = -not $needsContext -and
        ($old -match '^[\p{L}\p{M}]+$') -and ($new -match '^[\p{L}\p{M}]+$')
    $hasNumberOrSymbol = $old -match '[\d\W]' -or $new -match '[\d\W]'

    $oldContextStart = [Math]::Max(0, $oldStart - 2)
    $newContextStart = [Math]::Max(0, $newStart - 2)
    $oldContextEnd = [Math]::Min($oldTokens.Count, $oldEnd + 2)
    $newContextEnd = [Math]::Min($newTokens.Count, $newEnd + 2)
    $oldContext = Get-TokenSlice $dictated $oldTokens $oldContextStart $oldContextEnd
    $newContext = Get-TokenSlice $changed $newTokens $newContextStart $newContextEnd

    if ($needsContext -or $hasNumberOrSymbol) {
        $old = $oldContext
        $new = $newContext
    }
    if (-not (Test-SafeCorrectionValue $old) -or -not (Test-SafeCorrectionValue $new) -or $old -ceq $new) {
        return $null
    }
    if ($old -match '\d' -or $new -match '\d') {
        if (($old -split '\s+').Count -lt 3 -or ($new -split '\s+').Count -lt 3 -or
            $old -notmatch '\p{L}' -or $new -notmatch '\p{L}' -or
            $oldStart -lt 2 -or $newStart -lt 2 -or
            ($oldTokens.Count - $oldEnd) -lt 2 -or ($newTokens.Count - $newEnd) -lt 2) {
            return $null
        }
    }

    $candidate = [ordered]@{ term = $old; replacement = $new }
    if ($singleWords -and (Test-SafeCorrectionValue $oldContext) -and
        (Test-SafeCorrectionValue $newContext) -and $oldContext -cne $newContext) {
        $candidate.context_term = $oldContext
        $candidate.context_replacement = $newContext
    }
    return $candidate
}

function Get-CorrectionCandidates([string]$dictated, [string]$changed) {
    if ($dictated -ceq $changed) { return @() }
    $oldTokens = @(Get-CorrectionTokens $dictated)
    $newTokens = @(Get-CorrectionTokens $changed)
    if ($oldTokens.Count -eq 0 -or $newTokens.Count -eq 0) { return @() }

    $results = [System.Collections.Generic.List[object]]::new()
    $i = 0
    $j = 0
    $lookAhead = 18
    while ($i -lt $oldTokens.Count -or $j -lt $newTokens.Count) {
        if ($i -lt $oldTokens.Count -and $j -lt $newTokens.Count -and
            $oldTokens[$i].Value -ceq $newTokens[$j].Value) {
            $i++; $j++; continue
        }

        $oldStart = $i
        $newStart = $j
        $bestOldSkip = -1
        $bestNewSkip = -1
        $bestCost = [int]::MaxValue
        for ($oldSkip = 0; $oldSkip -le $lookAhead -and $i + $oldSkip -lt $oldTokens.Count; $oldSkip++) {
            for ($newSkip = 0; $newSkip -le $lookAhead -and $j + $newSkip -lt $newTokens.Count; $newSkip++) {
                if ($oldSkip -eq 0 -and $newSkip -eq 0) { continue }
                if ($oldTokens[$i + $oldSkip].Value -ceq $newTokens[$j + $newSkip].Value) {
                    $cost = $oldSkip + $newSkip
                    if ($cost -lt $bestCost) {
                        $bestCost = $cost
                        $bestOldSkip = $oldSkip
                        $bestNewSkip = $newSkip
                    }
                }
            }
        }

        if ($bestOldSkip -lt 0) {
            $oldEnd = $oldTokens.Count
            $newEnd = $newTokens.Count
            $i = $oldEnd
            $j = $newEnd
        } else {
            $oldEnd = $i + $bestOldSkip
            $newEnd = $j + $bestNewSkip
            $i = $oldEnd
            $j = $newEnd
        }
        $candidate = New-CorrectionCandidate $dictated $changed $oldTokens $newTokens $oldStart $oldEnd $newStart $newEnd
        if ($null -ne $candidate) { $results.Add($candidate) }
    }
    return @($results)
}

function Get-CorrectionCandidate([string]$dictated, [string]$changed) {
    $candidates = @(Get-CorrectionCandidates $dictated $changed)
    if ($candidates.Count -eq 0) { return $null }
    return $candidates[0]
}
