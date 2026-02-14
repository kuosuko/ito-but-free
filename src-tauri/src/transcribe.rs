use std::path::PathBuf;

use anyhow::{anyhow, Context, Result};
use reqwest::multipart;
use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct GroqTranscriptionResponse {
    text: String,
}

#[derive(Debug, Deserialize)]
struct GroqChatResponse {
    choices: Vec<GroqChoice>,
}

#[derive(Debug, Deserialize)]
struct GroqChoice {
    message: GroqMessage,
}

#[derive(Debug, Deserialize)]
struct GroqMessage {
    content: String,
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

pub async fn refine_transcript(api_key: String, transcript: String, custom_prompt: String, model: String) -> Result<String> {
    if api_key.trim().is_empty() {
        return Err(anyhow!("Missing Groq API key"));
    }

    let base_instruction = "Please improve the following transcript by fixing grammar, punctuation, and making it more readable while preserving the original meaning. Apply any style or language preferences specified in the next section.";
    
    let full_prompt = format!(
        "{}\n\nStyle/Language Preferences:\n{}\n\nOriginal Transcript:\n<transcript>\n{}\n</transcript>",
        base_instruction,
        if custom_prompt.trim().is_empty() { "No specific style preferences." } else { &custom_prompt },
        transcript
    );

    let client = reqwest::Client::new();
    let request_body = serde_json::json!({
        "model": model,
        "messages": [
            {
                "role": "system",
                "content": "You are a helpful assistant that refines and improves text transcripts."
            },
            {
                "role": "user",
                "content": full_prompt
            }
        ],
        "temperature": 0.1
    });

    let res = client
        .post("https://api.groq.com/openai/v1/chat/completions")
        .bearer_auth(api_key)
        .json(&request_body)
        .send()
        .await
        .context("Groq chat request failed")?;

    if !res.status().is_success() {
        let status = res.status();
        let body = res.text().await.unwrap_or_default();
        return Err(anyhow!("Groq refinement failed: {status} {body}"));
    }

    let parsed: GroqChatResponse = res
        .json()
        .await
        .context("Failed to parse Groq chat response JSON")?;
    
    if parsed.choices.is_empty() {
        return Err(anyhow!("No choices returned from Groq API"));
    }
    
    Ok(parsed.choices[0].message.content.clone())
}
