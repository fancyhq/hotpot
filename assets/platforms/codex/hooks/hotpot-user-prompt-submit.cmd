@echo off
REM Delegate Codex UserPromptSubmit payload handling to the Rust CLI.
hotpot hook codex user-prompt-submit
exit /b %ERRORLEVEL%
