#!/bin/sh
# Delegate Claude Code UserPromptSubmit payload handling to the Rust CLI.
exec hotpot hook claude user-prompt-submit
