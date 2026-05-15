@echo off
REM Delegate Claude Code SubagentStart payload handling to the Rust CLI.
hotpot hook claude subagent-start
exit /b %ERRORLEVEL%
