//! An upload filled straight into S3 through a presigned PUT URL, as the
//! relay behind API Gateway hands them out (RFC 0007, "How large changes go
//! to S3"; NOC-272): the backend takes it once its object holds exactly the
//! declared size, and not otherwise. Ignored unless an AWS Emulator runs on
//! this machine (docs/aws-emulator.md); the CI job `aws-emulator` runs it.

#[path = "backend/fresh.rs"]
mod fresh;

#[allow(dead_code)]
#[path = "emulator/common.rs"]
mod common;

use std::time::Duration;

use aws_sdk_s3::presigning::PresigningConfig;
use aws_sdk_s3::primitives::ByteStream;
use fresh::fresh;
use ncdb_core::ids::{MemberId, SpaceId};
use ncdb_relay::conformance::{Backend, block_on};
use ncdb_relay::store::{RelayStore, Token, Upload, UploadId};
use ncdb_relay_aws::upload_key;

const SPACE: SpaceId = SpaceId([0x5a; 16]);
const MEMBER: MemberId = MemberId([0x6b; 32]);

fn upload(n: u8, size: u64) -> Upload {
    Upload {
        id: UploadId([n; 16]),
        token: Token([n; 32]),
        space: SPACE,
        member: MEMBER,
        size,
        expires_at: u64::MAX / 2,
    }
}

#[test]
#[ignore = "needs the AWS Emulator (docs/aws-emulator.md)"]
fn an_upload_put_through_its_presigned_url_is_taken() {
    let aws = fresh();
    let store = aws.connect();
    let frame = vec![0x42; 20_000];
    let up = upload(1, frame.len() as u64);
    block_on(store.begin_upload(up.clone(), 0)).unwrap();

    let s3 = aws.s3();
    let presigned = aws
        .runtime
        .block_on(
            s3.put_object()
                .bucket(&aws.names.bucket)
                .key(upload_key(up.id))
                .content_length(frame.len() as i64)
                .presigned(PresigningConfig::expires_in(Duration::from_secs(900)).unwrap()),
        )
        .unwrap();
    let put = aws
        .runtime
        .block_on(
            common::https()
                .put(presigned.uri())
                .body(frame.clone())
                .send(),
        )
        .unwrap();
    assert!(put.status().is_success(), "{}", put.status());

    let taken = block_on(store.take_upload(SPACE, MEMBER, up.id)).unwrap();
    assert_eq!(taken.as_deref(), Some(&frame[..]));
    // Taken once.
    assert_eq!(
        block_on(store.take_upload(SPACE, MEMBER, up.id)).unwrap(),
        None
    );
}

#[test]
#[ignore = "needs the AWS Emulator (docs/aws-emulator.md)"]
fn an_upload_whose_object_has_another_size_or_none_is_not_taken() {
    let aws = fresh();
    let store = aws.connect();
    let short = upload(2, 20_000);
    let missing = upload(3, 20_000);
    block_on(store.begin_upload(short.clone(), 0)).unwrap();
    block_on(store.begin_upload(missing.clone(), 0)).unwrap();
    let s3 = aws.s3();
    aws.runtime
        .block_on(
            s3.put_object()
                .bucket(&aws.names.bucket)
                .key(upload_key(short.id))
                .body(ByteStream::from(vec![0x42; 19_999]))
                .send(),
        )
        .unwrap();

    assert_eq!(
        block_on(store.take_upload(SPACE, MEMBER, short.id)).unwrap(),
        None
    );
    assert_eq!(
        block_on(store.take_upload(SPACE, MEMBER, missing.id)).unwrap(),
        None
    );
    // Nor by another member.
    let other = upload(4, 20_000);
    block_on(store.begin_upload(other.clone(), 0)).unwrap();
    aws.runtime
        .block_on(
            s3.put_object()
                .bucket(&aws.names.bucket)
                .key(upload_key(other.id))
                .body(ByteStream::from(vec![0x42; 20_000]))
                .send(),
        )
        .unwrap();
    assert_eq!(
        block_on(store.take_upload(SPACE, MemberId([0x7c; 32]), other.id)).unwrap(),
        None
    );
}
