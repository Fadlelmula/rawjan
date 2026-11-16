@echo off
set "VCVARS=C:\Program Files (x86)\Microsoft Visual Studio\2022\BuildTools\VC\Auxiliary\Build\vcvars64.bat"
if not exist "%VCVARS%" (
  echo Visual Studio C++ build tools were not found. Install the Desktop development with C++ workload.
  exit /b 1
)
call "%VCVARS%" >nul
set "PATH=%USERPROFILE%\.cargo\bin;%PATH%"
if exist "C:\Strawberry\perl\bin" set "PATH=C:\Strawberry\perl\bin;C:\Strawberry\c\bin;%PATH%"
cd /d "%~dp0.."
call npm run tauri dev %*
