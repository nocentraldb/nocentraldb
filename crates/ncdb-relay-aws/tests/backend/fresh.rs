//! A fresh AWS backend on the AWS Emulator for each test: its own tables
//! and bucket, under a random prefix, removed when the test ends. It runs
//! only against an emulator on this machine (docs/aws-emulator.md): it
//! refuses an endpoint that is not a loopback address, and real
//! credentials in the environment.

use aws_config::SdkConfig;
use ncdb_relay::conformance::Backend;
use ncdb_relay_aws::{AwsStore, Names, schema};
use tokio::runtime::Runtime;

#[allow(dead_code)]
#[path = "../emulator/common.rs"]
mod common;

/// One backend: tables and a bucket that several handles open, and the
/// runtime the handles' clients run on.
pub struct Aws {
    pub runtime: Runtime,
    pub config: SdkConfig,
    pub names: Names,
}

impl Aws {
    /// The DynamoDB client of a new handle.
    pub fn db(&self) -> aws_sdk_dynamodb::Client {
        aws_sdk_dynamodb::Client::new(&self.config)
    }

    /// The S3 client of a new handle, path-style, as the emulator needs.
    pub fn s3(&self) -> aws_sdk_s3::Client {
        common::s3(&self.config)
    }
}

impl Backend for Aws {
    type Store = AwsStore;

    fn connect(&self) -> AwsStore {
        AwsStore::new(
            self.db(),
            self.s3(),
            self.names.clone(),
            self.runtime.handle().clone(),
        )
    }
}

impl Drop for Aws {
    fn drop(&mut self) {
        let (db, s3) = (self.db(), self.s3());
        let _ = self.runtime.block_on(schema::delete(&db, &s3, &self.names));
    }
}

/// A backend with new, empty tables and bucket.
pub fn fresh() -> Aws {
    common::endpoint();
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .expect("a runtime");
    let config = runtime.block_on(common::config());
    let mut prefix = [0u8; 6];
    getrandom::fill(&mut prefix).expect("a secure generator");
    let prefix: String = prefix.iter().map(|b| format!("{b:02x}")).collect();
    let names = Names::with_prefix(&format!("ncdb-{prefix}-"));
    let aws = Aws {
        runtime,
        config,
        names,
    };
    let (db, s3) = (aws.db(), aws.s3());
    aws.runtime
        .block_on(schema::create(&db, &s3, &aws.names))
        .expect("the tables and the bucket");
    aws
}
