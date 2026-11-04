# Windows display captures include be-host's own windows

On the Windows host, be-host's windows are not hidden from Windows.Graphics.Capture: a display
entry shows be-host's window too, as any screen share does.

## Why

A click on a display entry does what the same click on the real monitor would. Keeping be-host
in the capture means what the panel shows is what a click on it reaches; a click that lands on
be-host's mirror image does nothing (`FocusError::OnHost`), since it is on be-host itself.
