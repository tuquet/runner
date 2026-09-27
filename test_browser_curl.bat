@echo off
setlocal enabledelayedexpansion
title Tuquet Automa - Live Browser Workflow Trigger (CURL)

echo ============================================================
echo   TUQUET AUTOMA - LIVE BROWSER WORKFLOW TEST (CURL)
echo ============================================================
echo.

:: 1. Kiem tra Automa Core Daemon
echo [1/4] Kiem tra ket noi den Automa Daemon (http://127.0.0.1:8765)...
curl.exe -s -m 2 http://127.0.0.1:8765/api/v1/health > nul 2>&1
if %errorlevel% neq 0 (
    echo [*] Daemon chua chay. Dang khoi dong Automa Daemon tren Desktop...
    start "Tuquet Automa Daemon" automa.exe server -p 8765
    timeout /t 3 /nobreak > nul
) else (
    echo [OK] Daemon dang hoat dong binh thuong.
)

:: 2. Tao file payload JSON vao thu muc Temp (headless: false)
set "PAYLOAD_FILE=%TEMP%\automa_visual_test_%RANDOM%.json"
echo [2/4] Khoi tao workflow payload tai: %PAYLOAD_FILE%

(
echo {
echo   "workflowData": {
echo     "name": "Live Visual Browser Workflow",
echo     "drawflow": {
echo       "nodes": [
echo         {
echo           "id": "node-trigger",
echo           "label": "trigger",
echo           "data": { "type": "manual" }
echo         },
echo         {
echo           "id": "node-new-tab",
echo           "label": "new-tab",
echo           "data": { "url": "https://example.com", "active": true }
echo         },
echo         {
echo           "id": "node-delay",
echo           "label": "delay",
echo           "data": { "time": 10000 }
echo         }
echo       ],
echo       "edges": [
echo         {
echo           "id": "edge-1",
echo           "source": "node-trigger",
echo           "target": "node-new-tab",
echo           "sourceHandle": "node-trigger-output-1",
echo           "targetHandle": "node-new-tab-input-1"
echo         },
echo         {
echo           "id": "edge-2",
echo           "source": "node-new-tab",
echo           "target": "node-delay",
echo           "sourceHandle": "node-new-tab-output-1",
echo           "targetHandle": "node-delay-input-1"
echo         }
echo       ]
echo     }
echo   },
echo   "options": {
echo     "headless": false,
echo     "debug": true,
echo     "closeBrowserOnFinish": false
echo   }
echo }
) > "%PAYLOAD_FILE%"

:: 3. Gui request curl toi Daemon
echo.
echo [3/4] Gui curl POST den http://127.0.0.1:8765/api/v1/jobs...
echo ------------------------------------------------------------
set "RESPONSE_FILE=%TEMP%\automa_response_%RANDOM%.json"
curl.exe -s -X POST "http://127.0.0.1:8765/api/v1/jobs" ^
  -H "Content-Type: application/json" ^
  -d @"%PAYLOAD_FILE%" > "%RESPONSE_FILE%"

type "%RESPONSE_FILE%"
echo.
echo ------------------------------------------------------------

:: 4. Theo doi tien trinh va trang thai
echo.
echo [4/4] Theo doi tien trinh thuc thi cua trinh duyet...
powershell -NoProfile -Command ^
  "$resp = Get-Content '%RESPONSE_FILE%' | ConvertFrom-Json; " ^
  "if ($resp.jobId) { " ^
  "  Write-Host ('[*] Job ID: ' + $resp.jobId); " ^
  "  Write-Host '[*] Cua so Chromium GUI dang mo tren Desktop cua ban...'; " ^
  "  for ($i = 0; $i -lt 30; $i++) { " ^
  "    Start-Sleep -Seconds 1; " ^
  "    $st = curl.exe -s ('http://127.0.0.1:8765/api/v1/jobs/' + $resp.jobId + '/status') | ConvertFrom-Json; " ^
  "    Write-Host ('[*] Tien do job: ' + $st.status); " ^
  "    if ($st.status -eq 'completed' -or $st.status -eq 'error') { break; } " ^
  "  } " ^
  "}"

echo.
echo [SUCCESS] Hoan tat! Trinh duyet van duoc giu nguyen tren man hinh de ban quan sat.
echo Nhan phim bat ky de ket thuc script...
pause > nul

:: Don dep file temp
del /q "%PAYLOAD_FILE%" "%RESPONSE_FILE%" 2>nul
