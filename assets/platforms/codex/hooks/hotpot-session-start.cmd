@echo off
REM Delegate Codex SessionStart payload handling to the Rust CLI.
hotpot hook codex session-start
exit /b %ERRORLEVEL%
