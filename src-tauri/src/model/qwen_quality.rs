use std::path::{Path, PathBuf};

use futures_util::StreamExt;
use tauri::{AppHandle, Emitter};
use tokio::io::AsyncWriteExt;

use super::{is_model_downloaded, ModelEntry};
use crate::errors::{AppError, AppErrorCode};

const FILES: [(&str, &str); 6] = [
    ("model_1.7B/conv_frontend.onnx", "conv_frontend.onnx"),
    ("model_1.7B/encoder.int8.onnx", "encoder.int8.onnx"),
    ("model_1.7B/decoder.int8.onnx", "decoder.int8.onnx"),
    ("tokenizer/vocab.json", "tokenizer/vocab.json"),
    ("tokenizer/merges.txt", "tokenizer/merges.txt"),
    ("tokenizer/tokenizer_config.json", "tokenizer/tokenizer_config.json"),
];
static DOWNLOAD_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

struct Staging(PathBuf);

impl Drop for Staging {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

pub(super) async fn download(
    app: &AppHandle,
    root: &Path,
    entry: &ModelEntry,
    url_prefix: &str,
) -> Result<(), AppError> {
    let _lock = DOWNLOAD_LOCK.lock().await;
    if !is_model_downloaded(root, entry) {
        let stage = Staging(root.join(format!(".qwen-quality-{}", uuid::Uuid::new_v4())));
        tokio::fs::create_dir_all(stage.0.join("tokenizer"))
            .await
            .map_err(|error| AppError::new(AppErrorCode::ModelMkdirFailed).with_detail(error.to_string()))?;

        let client = reqwest::Client::new();
        emit_progress(app, &entry.id, "downloading", 0);
        for (index, (source, target)) in FILES.iter().enumerate() {
            let url = format!("{}/{}", url_prefix.trim_end_matches('/'), source);
            let destination = stage.0.join(target);
            download_file(&client, &url, &destination, |within| {
                let progress = (index as u32 * 100 + within) / FILES.len() as u32;
                emit_progress(app, &entry.id, "downloading", progress.min(99));
            })
            .await?;
        }

        tokio::fs::rename(&stage.0, root.join(&entry.id))
            .await
            .map_err(|error| AppError::new(AppErrorCode::ModelSaveFailed).with_detail(error.to_string()))?;
    }
    emit_progress(app, &entry.id, "completed", 100);
    Ok(())
}

async fn download_file(
    client: &reqwest::Client,
    url: &str,
    path: &Path,
    progress: impl Fn(u32),
) -> Result<(), AppError> {
    let response = client.get(url).send().await
        .and_then(reqwest::Response::error_for_status)
        .map_err(|error| AppError::new(AppErrorCode::ModelDownloadFailed).with_detail(error.to_string()))?;
    if response.headers().get(reqwest::header::CONTENT_TYPE)
        .and_then(|header| header.to_str().ok())
        .is_some_and(|kind| kind.starts_with("text/html"))
    {
        return Err(AppError::new(AppErrorCode::ModelDownloadFailed)
            .with_detail("The model server returned a web page"));
    }
    let expected = response.content_length();
    let mut bytes = response.bytes_stream();
    let save_error = |error: std::io::Error| {
        AppError::new(AppErrorCode::ModelSaveFailed).with_detail(error.to_string())
    };
    let mut file = tokio::fs::File::create(path).await.map_err(save_error)?;
    let mut received = 0u64;
    let mut last_progress = 0;
    while let Some(chunk) = bytes.next().await {
        let chunk = chunk.map_err(|error| {
            AppError::new(AppErrorCode::ModelDownloadFailed).with_detail(error.to_string())
        })?;
        file.write_all(&chunk).await.map_err(save_error)?;
        received += chunk.len() as u64;
        if let Some(size) = expected.filter(|size| *size > 0) {
            let percentage = ((received as f64 / size as f64 * 100.0) as u32).min(100);
            if percentage != last_progress {
                progress(percentage);
                last_progress = percentage;
            }
        }
    }
    file.flush().await.map_err(save_error)?;
    let minimum = if path.extension().is_some_and(|value| value == "onnx") {
        1024 * 1024
    } else {
        1024
    };
    if received < minimum || expected.is_some_and(|size| size != received) {
        return Err(AppError::new(AppErrorCode::ModelChecksumFailed)
            .with_param("model", super::QWEN_QUALITY_MODEL_ID));
    }
    progress(100);
    Ok(())
}

fn emit_progress(app: &AppHandle, model_id: &str, status: &str, progress: u32) {
    let _ = app.emit("model:download:progress", serde_json::json!({
        "model_id": model_id,
        "status": status,
        "progress": progress,
    }));
}
