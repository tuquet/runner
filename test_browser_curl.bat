@echo off
setlocal enabledelayedexpansion
title Tuquet Automa - Live Browser Workflow Trigger

echo ============================================================
echo   TUQUET AUTOMA - LIVE BROWSER WORKFLOW TEST (CURL)
echo ============================================================
echo.

:: 1. Kiem tra Automa Core Daemon
echo [1/4] Kiem tra ket noi den Automa Daemon (http://127.0.0.1:8765)...
curl.exe -s -m 2 http://127.0.0.1:8765/api/v1/health > nul 2>&1
if %errorlevel% neq 0 (
    echo [*] Daemon chua chay. Dang khoi dong Automa Daemon o background...
    start "" /b automa.exe server -p 8765
    timeout /t 3 /nobreak > nul
) else (
    echo [OK] Daemon dang hoat dong binh thuong.
)

:: 2. Tao file payload JSON vao thu muc Temp
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
echo           "data": { "time": 6000 }
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

:: 4. Thong bao ket qua
echo.
echo [4/4] [SUCCESS] Lenh da duoc tiep nhan!
echo [*] Trinh duyet Chromium GUI dang duoc mo truc tiep tren Desktop.
echo [*] Tab https://example.com se tu dong chay va duy tri hien thi.
echo.
echo Nhan phim bat ky de ket thuc...
pause > nul

:: Don dep file temp
del /q "%PAYLOAD_FILE%" "%RESPONSE_FILE%" 2>nul
