@echo off
powershell -NoProfile -Command "Get-CimInstance Win32_Process | Where-Object { $_.Name -match 'music-player|cargo|node' -and $_.CommandLine -match 'music-player' } | ForEach-Object { Stop-Process -Id $_.ProcessId -Force -ErrorAction SilentlyContinue }"
echo stopped.