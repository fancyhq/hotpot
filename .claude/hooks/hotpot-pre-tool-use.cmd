@echo off
REM Delegate Claude Code PreToolUse payload handling to the Rust CLI.
hotpot hook claude pre-tool-use
exit /b %ERRORLEVEL%
