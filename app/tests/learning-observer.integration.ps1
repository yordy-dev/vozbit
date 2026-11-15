$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName UIAutomationClient
Add-Type -AssemblyName UIAutomationTypes
Add-Type @'
using System;
using System.Runtime.InteropServices;
public static class VozbitLearningNative {
    [DllImport("user32.dll", CharSet = CharSet.Unicode, SetLastError = true)]
    public static extern bool SetWindowText(IntPtr hWnd, string text);
    [DllImport("user32.dll")]
    public static extern bool SetForegroundWindow(IntPtr hWnd);
    [DllImport("user32.dll")]
    public static extern bool ShowWindow(IntPtr hWnd, int command);
    [DllImport("user32.dll")]
    public static extern IntPtr SetFocus(IntPtr hWnd);
    [DllImport("user32.dll")]
    public static extern IntPtr GetForegroundWindow();
    [DllImport("user32.dll")]
    public static extern uint GetWindowThreadProcessId(IntPtr hWnd, IntPtr processId);
    [DllImport("user32.dll")]
    public static extern bool AttachThreadInput(uint first, uint second, bool attach);
}
'@

function Set-TestFieldFocus([IntPtr]$formHandle) {
    $foreground = [VozbitLearningNative]::GetForegroundWindow()
    $foregroundThread = [VozbitLearningNative]::GetWindowThreadProcessId($foreground, [IntPtr]::Zero)
    $targetThread = [VozbitLearningNative]::GetWindowThreadProcessId($formHandle, [IntPtr]::Zero)
    $attached = $foregroundThread -ne 0 -and $targetThread -ne 0 -and $foregroundThread -ne $targetThread
    if ($attached) { [void][VozbitLearningNative]::AttachThreadInput($foregroundThread, $targetThread, $true) }
    try {
        [void][VozbitLearningNative]::ShowWindow($formHandle, 5)
        [void][VozbitLearningNative]::SetForegroundWindow($formHandle)
    }
    finally {
        if ($attached) { [void][VozbitLearningNative]::AttachThreadInput($foregroundThread, $targetThread, $false) }
    }
}

$initial = 'En este parrafo largo quiero hablar de girosol, postgre y el Rook del cliente. Luego repetimos varias ideas para comprobar que una correccion distante no invalida las demas palabras del texto completo.'
$corrected = 'En este párrafo largo quiero hablar de girasoles, PostgreSQL y el RUC del cliente. Luego repetimos varias ideas para comprobar que una corrección distante no invalida las demás palabras del texto completo.'
$expected = @{
    parrafo = 'párrafo'
    girosol = 'girasoles'
    postgre = 'PostgreSQL'
    Rook = 'RUC'
    correccion = 'corrección'
    demas = 'demás'
}
$marker = Join-Path ([IO.Path]::GetTempPath()) "vozbit-learning-$PID.txt"
$target = $null
$observer = $null

try {
    $targetCode = @'
Add-Type -AssemblyName System.Windows.Forms
Add-Type -AssemblyName System.Drawing
$form = [Windows.Forms.Form]::new()
$form.Text = 'Vozbit learning test'
$form.Width = 720
$form.Height = 280
$box = [Windows.Forms.TextBox]::new()
$box.Multiline = $true
$box.Dock = [Windows.Forms.DockStyle]::Fill
$box.Text = $env:VOZBIT_TEST_INITIAL
$form.Controls.Add($box)
$form.Add_Shown({
    $form.Activate()
    $box.Focus()
    [IO.File]::WriteAllText($env:VOZBIT_TEST_MARKER, "$PID|$($form.Handle.ToInt64())|$($box.Handle.ToInt64())")
})
[Windows.Forms.Application]::Run($form)
'@
    $encoded = [Convert]::ToBase64String([Text.Encoding]::Unicode.GetBytes($targetCode))
    $targetInfo = [Diagnostics.ProcessStartInfo]::new()
    $targetInfo.FileName = (Join-Path $env:SystemRoot 'System32\WindowsPowerShell\v1.0\powershell.exe')
    $targetInfo.Arguments = "-NoProfile -STA -EncodedCommand $encoded"
    $targetInfo.UseShellExecute = $false
    $targetInfo.EnvironmentVariables['VOZBIT_TEST_INITIAL'] = $initial
    $targetInfo.EnvironmentVariables['VOZBIT_TEST_MARKER'] = $marker
    $target = [Diagnostics.Process]::Start($targetInfo)

    $deadline = [DateTime]::UtcNow.AddSeconds(10)
    while (-not (Test-Path -LiteralPath $marker)) {
        if ([DateTime]::UtcNow -gt $deadline) { throw 'El campo de prueba no obtuvo el foco' }
        Start-Sleep -Milliseconds 100
    }
    $markerParts = ([IO.File]::ReadAllText($marker)).Split('|')
    $targetPid = [int]$markerParts[0]
    $formHandle = [IntPtr][long]$markerParts[1]
    $boxHandle = [IntPtr][long]$markerParts[2]
    Set-TestFieldFocus $formHandle
    $testField = [System.Windows.Automation.AutomationElement]::FromHandle($boxHandle)
    if ($null -eq $testField) { throw 'No se encontró el campo editable de prueba' }
    $pidCondition = [System.Windows.Automation.PropertyCondition]::new(
        [System.Windows.Automation.AutomationElement]::ProcessIdProperty,
        $targetPid)
    $pidElements = [System.Windows.Automation.AutomationElement]::RootElement.FindAll(
        [System.Windows.Automation.TreeScope]::Descendants,
        $pidCondition)
    if ($pidElements.Count -eq 0) { throw "UI Automation no encontró elementos para PID $targetPid" }

    $observerInfo = [Diagnostics.ProcessStartInfo]::new()
    $observerInfo.FileName = (Join-Path $env:SystemRoot 'System32\WindowsPowerShell\v1.0\powershell.exe')
    $observerInfo.Arguments = '-NoProfile -NonInteractive -ExecutionPolicy Bypass -File "' +
        (Join-Path $PSScriptRoot '..\src-tauri\resources\observe-correction.ps1') + '"'
    $observerInfo.UseShellExecute = $false
    $observerInfo.CreateNoWindow = $true
    $observerInfo.RedirectStandardInput = $true
    $observerInfo.RedirectStandardOutput = $true
    $observerInfo.RedirectStandardError = $true
    $observerInfo.EnvironmentVariables['VOZBIT_LEARNING_TEST'] = '1'
    $observer = [Diagnostics.Process]::Start($observerInfo)
    Set-TestFieldFocus $formHandle
    $observer.StandardInput.WriteLine((@{
        pid = $targetPid
        owner = $PID
        text = $initial
        selection = $false
    } | ConvertTo-Json -Compress))
    $observer.StandardInput.Close()

    $readyTask = $observer.StandardError.ReadLineAsync()
    if (-not $readyTask.Wait(6000) -or $readyTask.Result -cne 'ready') {
        $readyState = if ($readyTask.IsCompleted) { $readyTask.Result } else { 'timeout' }
        throw "El observador no llegó al estado listo: $readyState"
    }
    Set-TestFieldFocus $formHandle
    if (-not [VozbitLearningNative]::SetWindowText($boxHandle, $corrected)) {
        throw 'No se pudo editar el campo de prueba'
    }

    $actual = @{}
    $readDeadline = [DateTime]::UtcNow.AddSeconds(8)
    $task = $observer.StandardOutput.ReadLineAsync()
    while ($actual.Count -lt $expected.Count -and [DateTime]::UtcNow -lt $readDeadline) {
        if (-not $task.Wait(1000)) { continue }
        $line = $task.Result
        if (-not [string]::IsNullOrWhiteSpace($line)) {
            $batch = $line | ConvertFrom-Json
            if ([string]$batch.original -cne $initial -or [string]$batch.edited -cne $corrected) {
                throw "El lote no conservó los textos original y corregido"
            }
            foreach ($candidate in @($batch.candidates)) {
                $actual[[string]$candidate.term] = [string]$candidate.replacement
            }
        }
        $task = $observer.StandardOutput.ReadLineAsync()
    }
    $actualJson = $actual | ConvertTo-Json -Compress
    if ($actual.Count -lt $expected.Count -and -not $observer.HasExited) {
        $observer.Kill()
        $observer.WaitForExit()
    }
    $observerError = $observer.StandardError.ReadToEnd().Trim()
    foreach ($term in $expected.Keys) {
        if (-not $actual.ContainsKey($term) -or $actual[$term] -cne $expected[$term]) {
            throw "El observador no detectó '$term' -> '$($expected[$term])'. Recibido: $actualJson. Estado: $observerError"
        }
    }
    "$($actual.Count) correcciones observadas de extremo a extremo"
}
finally {
    if ($null -ne $observer -and -not $observer.HasExited) { $observer.Kill() }
    if ($null -ne $target -and -not $target.HasExited) { $target.Kill() }
    if (Test-Path -LiteralPath $marker) { Remove-Item -LiteralPath $marker -Force }
}
