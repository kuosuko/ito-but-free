use std::path::PathBuf;

use anyhow::{anyhow, Context, Result};
use reqwest::multipart;
use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct GroqTranscriptionResponse {
    text: String,
}

pub async fn transcribe_groq(wav_path: PathBuf, api_key: String) -> Result<String> {
    if api_key.trim().is_empty() {
        return Err(anyhow!("Missing Groq API key"));
    }

    let bytes = tokio::fs::read(&wav_path)
        .await
        .with_context(|| format!("Failed to read audio file: {}", wav_path.display()))?;

    let file_part = multipart::Part::bytes(bytes)
        .file_name(
            wav_path
                .file_name()
                .and_then(|s| s.to_str())
                .unwrap_or("audio.wav")
                .to_string(),
        )
        .mime_str("audio/wav")?;

    // Groq OpenAI-compatible endpoint
    let form = multipart::Form::new()
        .text("model", "whisper-large-v3")
        .part("file", file_part);

    let client = reqwest::Client::new();
    let res = client
        .post("https://api.groq.com/openai/v1/audio/transcriptions")
        .bearer_auth(api_key)
        .multipart(form)
        .send()
        .await
        .context("Groq request failed")?;

    if !res.status().is_success() {
        let status = res.status();
        let body = res.text().await.unwrap_or_default();
        return Err(anyhow!("Groq transcription failed: {status} {body}"));
    }

    let parsed: GroqTranscriptionResponse = res
        .json()
        .await
        .context("Failed to parse Groq response JSON")?;
    Ok(parsed.text)
}
