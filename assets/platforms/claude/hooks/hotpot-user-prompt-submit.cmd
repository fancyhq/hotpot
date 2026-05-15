@echo off
REM Delegate Claude Code UserPromptSubmit payload handling to the Rust CLI.
hotpot hook claude user-prompt-submit
exit /b %ERRORLEVEL%
