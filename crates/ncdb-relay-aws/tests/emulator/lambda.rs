//! Lambda: creating a function and invoking it.

use super::common::{Outcome, failed};

/// A zip archive with one stored (uncompressed) file, enough for a Lambda
/// deployment package.
fn zip(name: &str, content: &[u8]) -> Vec<u8> {
    fn crc32(data: &[u8]) -> u32 {
        let mut crc = 0xffff_ffffu32;
        for byte in data {
            crc ^= u32::from(*byte);
            for _ in 0..8 {
                crc = (crc >> 1) ^ (0xedb8_8320 & (0u32.wrapping_sub(crc & 1)));
            }
        }
        !crc
    }
    let crc = crc32(content);
    let size = content.len() as u32;
    let name_len = name.len() as u16;
    let mut out = Vec::new();
    let local = |out: &mut Vec<u8>| {
        out.extend_from_slice(&0x0403_4b50u32.to_le_bytes());
        out.extend_from_slice(&[20, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
        out.extend_from_slice(&crc.to_le_bytes());
        out.extend_from_slice(&size.to_le_bytes());
        out.extend_from_slice(&size.to_le_bytes());
        out.extend_from_slice(&name_len.to_le_bytes());
        out.extend_from_slice(&0u16.to_le_bytes());
    };
    local(&mut out);
    out.extend_from_slice(name.as_bytes());
    out.extend_from_slice(content);
    let central = out.len() as u32;
    out.extend_from_slice(&0x0201_4b50u32.to_le_bytes());
    out.extend_from_slice(&[20, 0, 20, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
    out.extend_from_slice(&crc.to_le_bytes());
    out.extend_from_slice(&size.to_le_bytes());
    out.extend_from_slice(&size.to_le_bytes());
    out.extend_from_slice(&name_len.to_le_bytes());
    out.extend_from_slice(&[0; 12]);
    out.extend_from_slice(&0u32.to_le_bytes());
    out.extend_from_slice(name.as_bytes());
    let central_size = out.len() as u32 - central;
    out.extend_from_slice(&0x0605_4b50u32.to_le_bytes());
    out.extend_from_slice(&[0, 0, 0, 0, 1, 0, 1, 0]);
    out.extend_from_slice(&central_size.to_le_bytes());
    out.extend_from_slice(&central.to_le_bytes());
    out.extend_from_slice(&0u16.to_le_bytes());
    out
}

/// Lambda `CreateFunction` and `Invoke`, with a Python handler that echoes
/// its event.
pub async fn lambda(config: &aws_config::SdkConfig) -> (Outcome, Outcome) {
    let client = aws_sdk_lambda::Client::new(config);
    let code = zip(
        "handler.py",
        b"def handle(event, context):\n    return event\n",
    );
    let created = client
        .create_function()
        .function_name("relay")
        .runtime(aws_sdk_lambda::types::Runtime::Python312)
        .role("arn:aws:iam::000000000000:role/relay")
        .handler("handler.handle")
        .code(
            aws_sdk_lambda::types::FunctionCode::builder()
                .zip_file(aws_sdk_lambda::primitives::Blob::new(code))
                .build(),
        )
        .send()
        .await;
    let create = match created {
        Ok(_) => Outcome::Supported,
        Err(error) => return (failed(&error), Outcome::Differs("no function".into())),
    };
    // A new function is `Pending` until AWS has set it up, and AWS refuses
    // to invoke it until then with `ResourceConflictException`: wait for
    // `Active`, as a deployment does.
    let mut state = String::new();
    for _ in 0..60 {
        match client
            .get_function_configuration()
            .function_name("relay")
            .send()
            .await
        {
            Ok(out) => {
                state = out
                    .state()
                    .map(|s| s.as_str().to_owned())
                    .unwrap_or_default();
                if state == "Active" {
                    break;
                }
            }
            Err(error) => return (create, failed(&error)),
        }
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
    }
    if state != "Active" {
        return (
            create,
            Outcome::Differs(format!("the function stays {state:?}, never Active")),
        );
    }
    let invoke = match client
        .invoke()
        .function_name("relay")
        .payload(aws_sdk_lambda::primitives::Blob::new(
            br#"{"action":"ping"}"#.to_vec(),
        ))
        .send()
        .await
    {
        Ok(out) => match out.payload().map(|p| p.as_ref().to_vec()) {
            Some(p) if p == br#"{"action": "ping"}"# || p == br#"{"action":"ping"}"# => {
                Outcome::Supported
            }
            Some(p) => Outcome::Differs(format!(
                "payload {:?}{}",
                String::from_utf8_lossy(&p),
                out.function_error()
                    .map(|e| format!(", function error {e}"))
                    .unwrap_or_default()
            )),
            None => Outcome::Differs("no payload".into()),
        },
        Err(error) => failed(&error),
    };
    (create, invoke)
}
