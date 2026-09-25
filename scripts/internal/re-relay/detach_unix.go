//go:build !windows

package main

import (
	"os/exec"
	"syscall"
)

// The relay outlives the ./scripts/buck that started it, so it leaves that
// terminal's session: a Ctrl-C there is meant for buck2, not for it.
func detach(command *exec.Cmd) {
	command.SysProcAttr = &syscall.SysProcAttr{Setsid: true}
}
