use std::net::SocketAddr;

use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    sync::oneshot,
};

use super::*;

mod namespace_wakes_the_cluster_once_for_callers_together;
