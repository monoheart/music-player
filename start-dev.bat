@echo off
set RUSTUP_HOME=D:\_workspace2\.rustup
set CARGO_HOME=D:\_workspace2\.cargo
rem force build dir into project to override any inherited CARGO_TARGET_DIR (previously created C:\rs-target)
set CARGO_TARGET_DIR=D:\_workspace2\opencode\music-player\src-tauri\target
set LIB=C:\Program Files (x86)\Windows Kits\10\Lib\10.0.26100.0\um\x64;C:\Program Files (x86)\Windows Kits\10\Lib\10.0.26100.0\ucrt\x64;D:\_workspace2\VS2022\BuildTools\VC\Tools\MSVC\14.44.35207\lib\x64
set INCLUDE=C:\Program Files (x86)\Windows Kits\10\Include\10.0.26100.0\ucrt;C:\Program Files (x86)\Windows Kits\10\Include\10.0.26100.0\um;C:\Program Files (x86)\Windows Kits\10\Include\10.0.26100.0\shared;D:\_workspace2\VS2022\BuildTools\VC\Tools\MSVC\14.44.35207\include
cd /d D:\_workspace2\opencode\music-player
npm run tauri dev > dev.log 2> dev-err.log
