@AGENTS.md

- To wait for a background command, end your turn: you are re-invoked when it exits. This includes a foreground command that outlives its timeout and is moved to the background. Never loop reading its output file or sleeping to check on it. If a stop hook then tells you to commit uncommitted changes while the command is still running, end your turn again: the hook blocks only once.
