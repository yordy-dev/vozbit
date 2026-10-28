function Get-CorrectionCandidate([string]$dictated, [string]$changed) {
    if ($dictated -ceq $changed) { return $null }
    $a = 0
    while ($a -lt [Math]::Min($dictated.Length,$changed.Length) -and $dictated[$a] -ceq $changed[$a]) { $a++ }
    while ($a -gt 0 -and [char]::IsLetterOrDigit($dictated[$a-1])) { $a-- }
    $b = $dictated.Length; $c = $changed.Length
    while ($b -gt $a -and $c -gt $a -and $dictated[$b-1] -ceq $changed[$c-1]) { $b--; $c-- }
    while ($b -lt $dictated.Length -and [char]::IsLetterOrDigit($dictated[$b])) { $b++; if ($c -lt $changed.Length) { $c++ } }
    $old = $dictated.Substring($a,$b-$a).Trim()
    $new = $changed.Substring($a,$c-$a).Trim()
    if ($old.Length -lt 2 -or $new.Length -lt 1 -or $old -ceq $new) { return $null }
    if ($old -match '[\r\n@:\\]' -or $new -match '[\r\n@:\\]') { return $null }
    # Numbers, formulas and punctuation are only suggestions with surrounding context.
    if ($old -match '[\d\W]' -or $new -match '[\d\W]') {
        $before = $dictated.Substring(0,$a)
        $after = $dictated.Substring($b)
        $leftTokens = [regex]::Matches($before,'\S+\s*')
        $rightTokens = [regex]::Matches($after,'\s*\S+')
        $hasNumber = $old -match '\d' -or $new -match '\d'
        if ($hasNumber -and ($leftTokens.Count -eq 0 -or $rightTokens.Count -eq 0)) { return $null }
        if ($leftTokens.Count + $rightTokens.Count -lt 2) { return $null }
        $contextStart = if ($leftTokens.Count -gt 0) { $leftTokens[[Math]::Max(0,$leftTokens.Count-2)].Index } else { $a }
        $contextEnd = $b
        if ($rightTokens.Count -gt 0) {
            $rightEnd = $rightTokens[[Math]::Min(1,$rightTokens.Count-1)]
            $contextEnd = $b + $rightEnd.Index + $rightEnd.Length
        }
        $old = $dictated.Substring($contextStart,$contextEnd-$contextStart).Trim()
        $new = $changed.Substring($contextStart,$contextEnd-($b-$c)-$contextStart).Trim()
        if ($old -notmatch '\p{L}' -or $new -notmatch '\p{L}') { return $null }
    }
    if ($old.Length -gt 80 -or $new.Length -gt 80 -or $old -match '\d{6,}' -or $new -match '\d{6,}') { return $null }
    if (($old -split '\s+').Count -gt 7 -or ($new -split '\s+').Count -gt 7) { return $null }
    return @{term=$old;replacement=$new}
}
