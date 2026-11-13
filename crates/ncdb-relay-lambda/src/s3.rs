//! The URLs of large changes on AWS: presigned S3 URLs, as RFC 0007 says
//! ("How large changes go to S3"). An upload's frame goes to
//! `uploads/<upload id>` through a PUT URL valid for 15 minutes, with its
//! size fixed in the signature, so S3 refuses a body of any other length.
//! An entry's frame comes from `frames/<space id>/<change id>` through a
//! GET URL valid for 5 minutes, so large frames never pass through Lambda
//! on the way out. Presigning signs locally; it calls no AWS API.
//!
//! The relay core's tokens play no part here: the AWS backend's `tokens`
//! table, which serves them on a deployment that serves its own URLs, is
//! not used by a relay behind API Gateway.

use std::time::Duration;

use aws_sdk_s3::Client;
use aws_sdk_s3::presigning::PresigningConfig;
use ncdb_core::ids::SpaceId;
use ncdb_relay::store::{Entry, RelayStore, Token, UploadId};
use ncdb_relay::{DOWNLOAD_URL_VALID_FOR, ErrorCode, Relay, UPLOAD_URL_VALID_FOR};
use ncdb_relay_aws::{frame_key, upload_key};

use crate::handler::Urls;

/// The presigned URLs of the bucket `bucket`, signed with the client's
/// credentials.
#[derive(Clone)]
pub struct S3Urls {
    s3: Client,
    bucket: String,
}

impl std::fmt::Debug for S3Urls {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("S3Urls")
            .field("bucket", &self.bucket)
            .finish_non_exhaustive()
    }
}

impl S3Urls {
    /// The URLs of `bucket`, presigned by `s3`.
    pub fn new(s3: Client, bucket: impl Into<String>) -> S3Urls {
        S3Urls {
            s3,
            bucket: bucket.into(),
        }
    }
}

fn valid_for(ms: u64) -> Result<PresigningConfig, ErrorCode> {
    PresigningConfig::expires_in(Duration::from_millis(ms)).map_err(|_| ErrorCode::Unavailable)
}

impl Urls for S3Urls {
    async fn upload(&self, id: UploadId, _: Token, size: u64) -> Result<String, ErrorCode> {
        let size = i64::try_from(size).map_err(|_| ErrorCode::TooLarge)?;
        let presigned = self
            .s3
            .put_object()
            .bucket(&self.bucket)
            .key(upload_key(id))
            .content_length(size)
            .presigned(valid_for(UPLOAD_URL_VALID_FOR)?)
            .await
            .map_err(|_| ErrorCode::Unavailable)?;
        Ok(presigned.uri().to_owned())
    }

    async fn download<S: RelayStore>(
        &self,
        _: &Relay<S>,
        space: SpaceId,
        entry: &Entry,
    ) -> Result<String, ErrorCode> {
        let presigned = self
            .s3
            .get_object()
            .bucket(&self.bucket)
            .key(frame_key(space, entry.change))
            .presigned(valid_for(DOWNLOAD_URL_VALID_FOR)?)
            .await
            .map_err(|_| ErrorCode::Unavailable)?;
        Ok(presigned.uri().to_owned())
    }
}
