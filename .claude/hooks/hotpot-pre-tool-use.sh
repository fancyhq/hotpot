#!/bin/sh
# Delegate Claude Code PreToolUse payload handling to the Rust CLI.
exec hotpot hook claude pre-tool-use
