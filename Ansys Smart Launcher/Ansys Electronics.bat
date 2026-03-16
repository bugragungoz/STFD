@echo off
title Ansys Smart Launcher
color 0B
:: Author GitHub: https://github.com/bugragungoz

echo ===================================================
echo 1/3: INITIALIZING LICENSE SERVICES...
echo Executing service startup sequence.
echo ===================================================

:: Start services and print standardized status output
net start "ANSYS Licensing Tomcat" >nul 2>&1
echo [OK] ANSYS Licensing Tomcat started successfully.

net start "ANSYS, Inc. License Manager CVD" >nul 2>&1
echo [OK] ANSYS, Inc. License Manager CVD started successfully.

echo.
echo ===================================================
echo 2/3: SERVICES ONLINE. LAUNCHING ANSYS ELECTRONICS...
echo License services will be stopped automatically after process exit.
echo Keep this console session active.
echo ===================================================

:: Wait 3 seconds to allow license services to fully initialize
timeout /t 3 /nobreak >nul

:: Launch Ansys Electronics Desktop
start "" "C:\Program Files\AnsysEM\v241\Win64\ansysedt.exe"

:: Allow process startup stabilization interval
timeout /t 5 /nobreak >nul

:CheckProcess
:: Check silently whether ansysedt.exe is still running
tasklist /FI "IMAGENAME eq ansysedt.exe" 2>NUL | find /I /N "ansysedt.exe">NUL
if "%ERRORLEVEL%"=="0" (
    :: If running, wait 3 seconds and check again
    timeout /t 3 /nobreak >nul
    goto CheckProcess
)

echo.
echo ===================================================
echo 3/3: ANSYS PROCESS TERMINATED. STOPPING LICENSE SERVICES...
echo Releasing service-related system resources.
echo ===================================================

:: Stop services and print standardized status output
net stop "ANSYS Licensing Tomcat" >nul 2>&1
echo [OK] ANSYS Licensing Tomcat stopped successfully.

net stop "ANSYS, Inc. License Manager CVD" >nul 2>&1
echo [OK] ANSYS, Inc. License Manager CVD stopped successfully.

echo.
echo Shutdown sequence completed. Console will close in 3 seconds.
timeout /t 3 /nobreak >nul
exit