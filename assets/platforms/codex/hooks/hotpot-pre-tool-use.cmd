@echo off
REM Delegate Codex PreToolUse payload handling to the Rust CLI.
hotpot hook codex pre-tool-use
exit /b %ERRORLEVEL%
