//! "Ask AI": the user's spoken words are the prompt, highlighted text is the context.

use crate::settings::{AiProvider, AiSettings, WritingProfile};
use serde_json::{json, Value};
use std::time::Duration;

const KEYCHAIN_SERVICE: &str = "com.soonbuilt.blurt";

const ASK_SYSTEM: &str = "You are Tally, the helper inside Blurt, which the user talks to from any app on their computer. \
They held a key and spoke an instruction; your reply is typed straight into the app they're using and copied to \
their clipboard. If CONTEXT is given, it's text they highlighted: apply the instruction to it and reply with only \
the new text, which will replace the highlight. If there's no context: when they ask you to write something, write \
it; when they ask a question, answer it briefly. \
Reply with the final text only: no preamble, no quotes around it, no notes about what you changed. \
Use plain text, and only use lists or line breaks when the instruction calls for them. \
Keep the language of the context, or of the instruction when there's no context. \
The instruction came from speech recognition, so read past small transcription mistakes. \
If RECENT is given, it's what you and the user did in the last few minutes: use it to understand follow-ups like \
\"make it shorter\" or \"now in Spanish\", but ignore it when the new instruction is unrelated.";

const POLISH_SYSTEM: &str = "You tidy up dictated text. Rewrite the user's dictation into clear, well-punctuated writing \
that keeps their meaning, tone and wording as much as possible. Fix obvious speech-recognition mistakes, remove \
false starts and repetition, and split run-on sentences. Don't add anything new. Reply with the rewritten text only.";

pub fn api_key(provider: AiProvider) -> Option<String> {
    let name = match provider {
        AiProvider::Anthropic => "anthropic",
        AiProvider::Openai => "openai",
        _ => return None,
    };
    keyring::Entry::new(KEYCHAIN_SERVICE, name)
        .ok()?
        .get_password()
        .ok()
}

pub fn set_api_key(provider: AiProvider, key: &str) -> anyhow::Result<()> {
    let name = match provider {
        AiProvider::Anthropic => "anthropic",
        AiProvider::Openai => "openai",
        _ => anyhow::bail!("this provider doesn't use an API key"),
    };
    let entry = keyring::Entry::new(KEYCHAIN_SERVICE, name)?;
    if key.trim().is_empty() {
        let _ = entry.delete_credential();
    } else {
        entry.set_password(key.trim())?;
    }
    Ok(())
}

/// Runs a spoken instruction, optionally against highlighted text. `tone` is how the user
/// sounded, when Blurt could tell; `recent` is short-term memory of the last few exchanges.
pub async fn ask(
    s: &AiSettings,
    profile: &WritingProfile,
    instruction: &str,
    context: Option<&str>,
    tone: Option<crate::voice::tone::Tone>,
    recent: Option<&str>,
) -> anyhow::Result<String> {
    let mut user = String::new();
    if let Some(r) = recent {
        user.push_str(&format!("RECENT (oldest first):\n{r}\n\n"));
    }
    user.push_str(&format!("INSTRUCTION: {instruction}"));
    if let Some(c) = context {
        user.push_str(&format!("\n\nCONTEXT:\n{c}"));
    }
    if let Some(t) = tone.filter(|t| *t != crate::voice::tone::Tone::Calm) {
        user.push_str(&format!(
            "\n\nTONE: the user sounded {} as they said this. Let it shape your wording (calm and kind if they're \
             frustrated, down or stressed; match their energy if they're upbeat), but don't mention it.",
            t.describe()
        ));
    }
    complete(s, &system_with_style(ASK_SYSTEM, profile, s), &user).await
}

/// Tidies dictation into good writing (the "Polished" dictation style).
pub async fn polish(s: &AiSettings, profile: &WritingProfile, dictation: &str) -> anyhow::Result<String> {
    complete(s, &system_with_style(POLISH_SYSTEM, profile, s), dictation).await
}

fn system_with_style(base: &str, profile: &WritingProfile, s: &AiSettings) -> String {
    let mut guidance = profile.guidance();
    if !s.style.trim().is_empty() {
        guidance.push(' ');
        guidance.push_str(s.style.trim());
    }
    if guidance.trim().is_empty() {
        base.to_string()
    } else {
        format!("{base}\n\nABOUT THE USER AND HOW THEY WRITE (follow unless the instruction says otherwise): {}", guidance.trim())
    }
}

async fn complete(s: &AiSettings, system: &str, user: &str) -> anyhow::Result<String> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(60))
        .build()?;
    let out = match s.provider {
        AiProvider::Apple => crate::apple::complete(system, user).await?,
        AiProvider::Ollama => {
            let body = json!({
                "model": s.ollama_model,
                "stream": false,
                "messages": [{"role": "system", "content": system}, {"role": "user", "content": user}],
            });
            let url = format!("{}/api/chat", s.ollama_url.trim_end_matches('/'));
            let resp = client.post(url).json(&body).send().await.map_err(|_| {
                anyhow::anyhow!(
                    "Ollama isn't running. Start Ollama, or pick another AI engine in Settings."
                )
            })?;
            let v: Value = checked(resp).await?;
            v["message"]["content"]
                .as_str()
                .unwrap_or_default()
                .to_string()
        }
        AiProvider::Anthropic => {
            let key = api_key(AiProvider::Anthropic)
                .ok_or_else(|| anyhow::anyhow!("Add your Anthropic API key in Settings."))?;
            // Short rewrites don't need deep reasoning: low effort keeps it quick.
            let body = json!({
                "model": s.anthropic_model,
                "max_tokens": 4096,
                "system": system,
                "output_config": {"effort": "low"},
                "messages": [{"role": "user", "content": user}],
            });
            let resp = client
                .post("https://api.anthropic.com/v1/messages")
                .header("x-api-key", key)
                .header("anthropic-version", "2023-06-01")
                .json(&body)
                .send()
                .await?;
            let v: Value = checked(resp).await?;
            if v["stop_reason"] == "refusal" {
                anyhow::bail!("Claude declined that request.");
            }
            v["content"]
                .as_array()
                .map(|blocks| {
                    blocks
                        .iter()
                        .filter(|b| b["type"] == "text")
                        .filter_map(|b| b["text"].as_str())
                        .collect::<Vec<_>>()
                        .join("")
                })
                .unwrap_or_default()
        }
        AiProvider::Openai => {
            let key = api_key(AiProvider::Openai)
                .ok_or_else(|| anyhow::anyhow!("Add your API key in Settings."))?;
            let body = json!({
                "model": s.openai_model,
                "messages": [{"role": "system", "content": system}, {"role": "user", "content": user}],
            });
            let url = format!("{}/chat/completions", s.openai_url.trim_end_matches('/'));
            let resp = client.post(url).bearer_auth(key).json(&body).send().await?;
            let v: Value = checked(resp).await?;
            v["choices"][0]["message"]["content"]
                .as_str()
                .unwrap_or_default()
                .to_string()
        }
    };
    let out = out.trim().to_string();
    if out.is_empty() {
        anyhow::bail!("The AI came back empty. Try again?");
    }
    Ok(out)
}

async fn checked(resp: reqwest::Response) -> anyhow::Result<Value> {
    let status = resp.status();
    let v: Value = resp.json().await.unwrap_or(Value::Null);
    if !status.is_success() {
        let msg = v["error"]["message"]
            .as_str()
            .or(v["error"].as_str())
            .unwrap_or("request failed");
        anyhow::bail!("{} ({})", msg, status.as_u16());
    }
    Ok(v)
}
