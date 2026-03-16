@echo off
title Multisim Smart Launcher
color 0E
:: Author GitHub: https://github.com/bugragungoz

echo ===================================================
echo 1/3: INITIALIZING NI SERVICES...
echo Executing service startup sequence.
echo ===================================================

net start "NI Authentication Service" >nul 2>&1
echo [OK] NI Authentication Service started.
net start "NI Domain Service" >nul 2>&1
echo [OK] NI Domain Service started.
net start "NI mDNS Responder Service" >nul 2>&1
echo [OK] NI mDNS Responder Service started.
net start "NI Network Discovery" >nul 2>&1
echo [OK] NI Network Discovery started.
net start "NI PSP Service Locator" >nul 2>&1
echo [OK] NI PSP Service Locator started.
net start "NI Service Locator" >nul 2>&1
echo [OK] NI Service Locator started.
net start "NI Time Synchronization" >nul 2>&1
echo [OK] NI Time Synchronization started.

echo.
echo ===================================================
echo 2/3: SERVICES ONLINE. LAUNCHING MULTISIM...
echo NI services will be stopped automatically after process exit.
echo Keep this console session active.
echo ===================================================

timeout /t 3 /nobreak >nul

start "" "C:\Program Files (x86)\National Instruments\Circuit Design Suite 14.3\multisim.exe"

timeout /t 5 /nobreak >nul

:CheckProcess
tasklist /FI "IMAGENAME eq multisim.exe" 2>NUL | find /I /N "multisim.exe">NUL
if "%ERRORLEVEL%"=="0" (
    timeout /t 3 /nobreak >nul
    goto CheckProcess
)

echo.
echo ===================================================
echo 3/3: MULTISIM PROCESS TERMINATED. STOPPING NI SERVICES...
echo Releasing service-related system resources.
echo ===================================================

:: The /y switch automatically confirms dependent service stop prompts
net stop "NI Authentication Service" /y >nul 2>&1
echo [OK] NI Authentication Service stopped.
net stop "NI Domain Service" /y >nul 2>&1
echo [OK] NI Domain Service stopped.
net stop "NI mDNS Responder Service" /y >nul 2>&1
echo [OK] NI mDNS Responder Service stopped.
net stop "NI Network Discovery" /y >nul 2>&1
echo [OK] NI Network Discovery stopped.
net stop "NI PSP Service Locator" /y >nul 2>&1
echo [OK] NI PSP Service Locator stopped.
net stop "NI Service Locator" /y >nul 2>&1
echo [OK] NI Service Locator stopped.
net stop "NI Time Synchronization" /y >nul 2>&1
echo [OK] NI Time Synchronization stopped.

echo.
echo Shutdown sequence completed. Console will close in 3 seconds.
timeout /t 3 /nobreak >nul
exit