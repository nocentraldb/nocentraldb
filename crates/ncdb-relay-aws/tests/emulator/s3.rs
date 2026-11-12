//! The S3 operations of RFC 0007's frames and uploads, path-style: put,
//! get, copy and delete, presigned PUT with a fixed length and presigned
//! GET, a lifecycle rule and CORS.

use std::time::Duration;

use aws_sdk_s3::presigning::PresigningConfig;
use aws_sdk_s3::primitives::ByteStream;
use aws_sdk_s3::types::{
    BucketLifecycleConfiguration, CorsConfiguration, CorsRule, ExpirationStatus,
    LifecycleExpiration, LifecycleRule, LifecycleRuleFilter,
};

use super::common::{Outcome, failed};

const BUCKET: &str = "frames";

/// `PutObject` and `GetObject`, path-style.
pub async fn put_get(s3: &aws_sdk_s3::Client) -> Outcome {
    if let Err(error) = s3.create_bucket().bucket(BUCKET).send().await {
        return failed(&error);
    }
    if let Err(error) = s3
        .put_object()
        .bucket(BUCKET)
        .key("frames/s1/c1")
        .body(ByteStream::from_static(b"frame"))
        .send()
        .await
    {
        return failed(&error);
    }
    match s3
        .get_object()
        .bucket(BUCKET)
        .key("frames/s1/c1")
        .send()
        .await
    {
        Ok(out) => match out.body.collect().await {
            Ok(body) if body.clone().into_bytes().as_ref() == b"frame" => Outcome::Supported,
            Ok(_) => Outcome::Differs("another body".into()),
            Err(error) => Outcome::Differs(format!("the body: {error}")),
        },
        Err(error) => failed(&error),
    }
}

/// `CopyObject`, as an upload becomes a frame.
pub async fn copy(s3: &aws_sdk_s3::Client) -> Outcome {
    if let Err(error) = s3
        .copy_object()
        .bucket(BUCKET)
        .copy_source(format!("{BUCKET}/frames/s1/c1"))
        .key("frames/s1/c2")
        .send()
        .await
    {
        return failed(&error);
    }
    match s3
        .get_object()
        .bucket(BUCKET)
        .key("frames/s1/c2")
        .send()
        .await
    {
        Ok(out) => match out.body.collect().await {
            Ok(body) if body.clone().into_bytes().as_ref() == b"frame" => Outcome::Supported,
            _ => Outcome::Differs("the copy has another body".into()),
        },
        Err(error) => failed(&error),
    }
}

/// `DeleteObject`: a get afterwards is `NoSuchKey`.
pub async fn delete(s3: &aws_sdk_s3::Client) -> Outcome {
    if let Err(error) = s3
        .delete_object()
        .bucket(BUCKET)
        .key("frames/s1/c2")
        .send()
        .await
    {
        return failed(&error);
    }
    match s3
        .get_object()
        .bucket(BUCKET)
        .key("frames/s1/c2")
        .send()
        .await
    {
        Err(error) if error.as_service_error().is_some_and(|e| e.is_no_such_key()) => {
            Outcome::Supported
        }
        Err(error) => failed(&error),
        Ok(_) => Outcome::Differs("the object is still there".into()),
    }
}

/// A presigned PUT with the content length fixed in the signature: the
/// right length is stored, another length is refused.
pub async fn presigned_put(s3: &aws_sdk_s3::Client, https: &reqwest::Client) -> Outcome {
    let body = b"upload of eleven";
    let presigned = match s3
        .put_object()
        .bucket(BUCKET)
        .key("uploads/u1")
        .content_length(body.len() as i64)
        .presigned(PresigningConfig::expires_in(Duration::from_secs(900)).expect("15 minutes"))
        .await
    {
        Ok(p) => p,
        Err(error) => return Outcome::Differs(format!("presigning: {error}")),
    };
    let send = |bytes: &'static [u8]| {
        let mut request = https.put(presigned.uri());
        for (name, value) in presigned.headers() {
            if !name.eq_ignore_ascii_case("content-length") {
                request = request.header(name, value);
            }
        }
        request.body(bytes).send()
    };
    match send(body).await {
        Ok(r) if r.status().as_u16() == 501 => return Outcome::NotImplemented,
        Ok(r) if r.status().is_success() => {}
        Ok(r) => return Outcome::Differs(format!("the right length got {}", r.status())),
        Err(error) => return Outcome::Differs(format!("no answer: {error}")),
    }
    match send(b"a longer body than was signed").await {
        Ok(r) if r.status().is_success() => {
            Outcome::Differs("a body of another length was stored".into())
        }
        Ok(_) => Outcome::Supported,
        Err(error) => Outcome::Differs(format!("no answer: {error}")),
    }
}

/// A presigned GET.
pub async fn presigned_get(s3: &aws_sdk_s3::Client, https: &reqwest::Client) -> Outcome {
    let presigned = match s3
        .get_object()
        .bucket(BUCKET)
        .key("frames/s1/c1")
        .presigned(PresigningConfig::expires_in(Duration::from_secs(300)).expect("5 minutes"))
        .await
    {
        Ok(p) => p,
        Err(error) => return Outcome::Differs(format!("presigning: {error}")),
    };
    match https.get(presigned.uri()).send().await {
        Ok(r) if r.status().as_u16() == 501 => Outcome::NotImplemented,
        Ok(r) if r.status().is_success() => match r.bytes().await {
            Ok(b) if b.as_ref() == b"frame" => Outcome::Supported,
            _ => Outcome::Differs("another body".into()),
        },
        Ok(r) => Outcome::Differs(format!("status {}", r.status())),
        Err(error) => Outcome::Differs(format!("no answer: {error}")),
    }
}

/// A lifecycle rule that expires uploads after one day.
pub async fn lifecycle(s3: &aws_sdk_s3::Client) -> Outcome {
    let rule = LifecycleRule::builder()
        .id("uploads")
        .status(ExpirationStatus::Enabled)
        .filter(LifecycleRuleFilter::builder().prefix("uploads/").build())
        .expiration(LifecycleExpiration::builder().days(1).build())
        .build()
        .expect("a lifecycle rule");
    let config = BucketLifecycleConfiguration::builder()
        .rules(rule)
        .build()
        .expect("a lifecycle configuration");
    if let Err(error) = s3
        .put_bucket_lifecycle_configuration()
        .bucket(BUCKET)
        .lifecycle_configuration(config)
        .send()
        .await
    {
        return failed(&error);
    }
    match s3
        .get_bucket_lifecycle_configuration()
        .bucket(BUCKET)
        .send()
        .await
    {
        Ok(out) if out.rules().len() == 1 => Outcome::Supported,
        Ok(out) => Outcome::Differs(format!("{} rules read back", out.rules().len())),
        Err(error) => failed(&error),
    }
}

/// A CORS configuration, for browsers that put and get frames.
pub async fn cors(s3: &aws_sdk_s3::Client) -> Outcome {
    let rule = CorsRule::builder()
        .allowed_methods("GET")
        .allowed_methods("PUT")
        .allowed_origins("*")
        .build()
        .expect("a CORS rule");
    let config = CorsConfiguration::builder()
        .cors_rules(rule)
        .build()
        .expect("a CORS configuration");
    if let Err(error) = s3
        .put_bucket_cors()
        .bucket(BUCKET)
        .cors_configuration(config)
        .send()
        .await
    {
        return failed(&error);
    }
    match s3.get_bucket_cors().bucket(BUCKET).send().await {
        Ok(out) if out.cors_rules().len() == 1 => Outcome::Supported,
        Ok(out) => Outcome::Differs(format!("{} rules read back", out.cors_rules().len())),
        Err(error) => failed(&error),
    }
}
