@echo off
setlocal enabledelayedexpansion
title Tuquet Automa - Direct Visual Browser Run

echo ============================================================
echo   TUQUET AUTOMA - DIRECT VISUAL BROWSER WORKFLOW
echo ============================================================
echo.
echo [*] Launching Chromium GUI directly and executing workflow...
echo [*] Mode: GUI (Headless: false)
echo.

set "WORKFLOW_FILE=%~dp0fixtures\test_browser_workflow.json"

automa.exe run -w "%WORKFLOW_FILE%"

echo.
echo ============================================================
echo [*] Execution completed.
echo ============================================================
echo.
pause
