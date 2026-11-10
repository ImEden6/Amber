@echo off
cd /d "%~dp0"
echo Starting Amber Eye Care (Optimized Release)...
start "" "%~dp0iris-app\src-tauri\target\release\amber-app.exe"
exit
