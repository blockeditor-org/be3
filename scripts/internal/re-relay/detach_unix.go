//go:build !windows

package main

import (
	"os/exec"
	"syscall"
)

// The relay outlives the ./scripts/bazel that started it, so it leaves that
// terminal's session: a Ctrl-C there is meant for Bazel, not for it.
func detach(command *exec.Cmd) {
	command.SysProcAttr = &syscall.SysProcAttr{Setsid: true}
}
