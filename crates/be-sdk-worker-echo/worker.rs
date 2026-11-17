//! The echo worker's module: its entry point, which buck links as :module (`worker()`).

be_sdk_worker::worker!(be_sdk_worker_echo::Echo);
