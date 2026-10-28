$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot '..\src-tauri\resources\correction-diff.ps1')
$cases = @(
    @{ Before='El Rook viene mañana.'; After='El RUC viene mañana.'; Term='Rook'; Replacement='RUC' },
    @{ Before='Vi un girosol amarillo.'; After='Vi un girasoles amarillo.'; Term='girosol'; Replacement='girasoles' },
    @{ Before='Vi un giresel amarillo.'; After='Vi un girasoles amarillo.'; Term='giresel'; Replacement='girasoles' },
    @{ Before='La ecuación es 13x + 4 - 13x = 0.'; After='La ecuación es 3x + 4 - 13x = 0.'; Term='ecuación es 13x + 4'; Replacement='ecuación es 3x + 4' },
    @{ Before='Hola amigo cómo estás.'; After='Hola, amigo cómo estás.'; Term='Hola amigo cómo'; Replacement='Hola, amigo cómo' },
    @{ Before='RUC 10409027276 corresponde.'; After='RUC 10409027270 corresponde.'; Term=$null; Replacement=$null },
    @{ Before='el número 13x'; After='el número 3x'; Term=$null; Replacement=$null }
)
foreach ($case in $cases) {
    $result = Get-CorrectionCandidate $case.Before $case.After
    if ($null -eq $case.Term) {
        if ($null -ne $result) { throw "Se aceptó una corrección insegura: $($case.Before)" }
    } elseif ($null -eq $result -or $result.term -cne $case.Term -or $result.replacement -cne $case.Replacement) {
        throw "Corrección inesperada: $($case.Before) -> $($result | ConvertTo-Json -Compress)"
    }
}
$first = 'El Rook viene mañana y está mal.'
$second = 'El RUC viene mañana y está mal.'
$third = 'El RUC viene mañana y está bien.'
if ((Get-CorrectionCandidate $first $second).replacement -cne 'RUC') { throw 'No se detectó la primera corrección' }
if ((Get-CorrectionCandidate $second $third).replacement -cne 'bien') { throw 'No se detectó la segunda corrección' }
"$($cases.Count + 2) pruebas de correcciones aprobadas"
