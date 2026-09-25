package main

import (
	"os/exec"
	"syscall"
)

const createNewProcessGroup = 0x00000200

func detach(command *exec.Cmd) {
	command.SysProcAttr = &syscall.SysProcAttr{CreationFlags: createNewProcessGroup}
}
