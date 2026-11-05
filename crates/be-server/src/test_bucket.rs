//! An S3 server to keep a test server's objects in: what tests of be-server, and of the
//! clients that move object bytes through it, run instead of a cloud bucket.

use std::{net::SocketAddr, path::PathBuf};

use be_block_store::Hash;
use uuid::Uuid;

use crate::objects::{Bucket, BucketConfig};

/// An S3 server (`s3s` over `s3s-fs`) on an ephemeral port, serving one bucket from a
/// temporary directory, on a runtime of its own so blocking requests from any test reach it.
/// Dropping it stops the server and removes the directory.
pub struct TestBucket {
    pub address: SocketAddr,
    pub directory: PathBuf,
    runtime: Option<tokio::runtime::Runtime>,
}

impl TestBucket {
    pub const NAME: &str = "objects";
    pub const REGION: &str = "test-region";
    const ACCESS_KEY: &str = "test-access";
    const SECRET_KEY: &str = "test-secret";

    pub fn start() -> Self {
        use hyper_util::{
            rt::{TokioExecutor, TokioIo},
            server::conn::auto::Builder,
        };
        let directory = std::env::temp_dir().join(format!("be-server-bucket-{}", Uuid::new_v4()));
        std::fs::create_dir_all(directory.join(Self::NAME)).unwrap();
        let service = {
            let mut builder =
                s3s::service::S3ServiceBuilder::new(s3s_fs::FileSystem::new(&directory).unwrap());
            builder.set_auth(s3s::auth::SimpleAuth::from_single(
                Self::ACCESS_KEY,
                Self::SECRET_KEY,
            ));
            builder.build()
        };
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let address = listener.local_addr().unwrap();
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .enable_all()
            .build()
            .unwrap();
        runtime.spawn(async move {
            let listener = tokio::net::TcpListener::from_std(listener).unwrap();
            loop {
                let Ok((socket, _)) = listener.accept().await else {
                    continue;
                };
                let service = service.clone();
                tokio::spawn(async move {
                    let _ = Builder::new(TokioExecutor::new())
                        .serve_connection(TokioIo::new(socket), service)
                        .await;
                });
            }
        });
        Self {
            address,
            directory,
            runtime: Some(runtime),
        }
    }

    pub fn bucket(&self) -> Bucket {
        Bucket::new(
            &format!("http://{}", self.address),
            Self::REGION,
            Self::NAME,
            Self::ACCESS_KEY,
            Self::SECRET_KEY,
        )
        .unwrap()
    }

    pub fn config(&self, cache_bytes: u64) -> BucketConfig {
        BucketConfig {
            bucket: self.bucket(),
            cache_bytes,
        }
    }

    /// Where the bucket keeps `hash`'s bytes, to tamper with.
    pub fn file(&self, hash: Hash) -> PathBuf {
        self.directory.join(Self::NAME).join(Bucket::key(hash))
    }
}

impl Drop for TestBucket {
    fn drop(&mut self) {
        if let Some(runtime) = self.runtime.take() {
            runtime.shutdown_background();
        }
        let _ = std::fs::remove_dir_all(&self.directory);
    }
}
