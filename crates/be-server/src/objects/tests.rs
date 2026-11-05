use std::{net::SocketAddr, path::PathBuf, time::Duration};

use be_block_store::{Hash, ObjectStore};
use uuid::Uuid;

use super::*;

mod a_bucket_store_round_trips_objects;
mod a_presigned_put_takes_only_the_object_it_was_signed_for;
mod a_tampered_object_reads_as_missing;
mod the_cache_in_front_of_a_bucket_keeps_only_its_limit;

/// An S3 server (`s3s` over `s3s-fs`) on an ephemeral port, serving one bucket from a
/// temporary directory, on a runtime of its own so blocking requests from any test reach it.
/// Dropping it stops the server and removes the directory.
pub(crate) struct TestBucket {
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

fn agent() -> ureq::Agent {
    ureq::AgentBuilder::new()
        .timeout(Duration::from_secs(30))
        .build()
}
