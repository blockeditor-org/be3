# A GPU device per guest

Depends on: plans/host_as_platform_adapter.md.

Natively every app draws on be-host's own device (guides/apps/framework.md); on the web each app
worker already has its own. Giving every guest but the root its own device natively would make a
lost device take down only that guest. Its textures would then reach the host's device through
external memory, which is platform-specific: dma-buf on Linux (be-lib-dmabuf), shared handles on
Windows, `AHardwareBuffer` on Android and `IOSurface` on macOS, each with the fences the import
needs, plus memory per device. Set aside until the user picks it up.
