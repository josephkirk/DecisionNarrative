//! AI Generators: LiteLLM text generation and ComfyUI image generation clients.

use std::collections::BTreeMap;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

#[derive(Clone, Debug, Deserialize)]
pub struct LiteLlmRequest {
    pub prompt: String,
    #[serde(default = "default_litellm_model")]
    pub model: String,
    #[serde(default)]
    pub system: Option<String>,
    #[serde(default = "default_temperature")]
    pub temperature: f64,
    #[serde(default = "default_max_tokens")]
    pub max_tokens: u32,
}

fn default_litellm_model() -> String { "gpt-4o-mini".into() }
fn default_temperature() -> f64 { 0.7 }
fn default_max_tokens() -> u32 { 1024 }

#[derive(Clone, Debug, Serialize)]
pub struct LiteLlmResponse {
    pub text: String,
    pub model: String,
}

#[derive(Clone, Debug, Deserialize)]
pub struct ComfyPromptRequest {
    pub prompt: String,
    #[serde(default)]
    pub negative: Option<String>,
    #[serde(default = "default_comfy_seed")]
    pub seed: u64,
    #[serde(default = "default_comfy_dimension")]
    pub width: u32,
    #[serde(default = "default_comfy_dimension")]
    pub height: u32,
}

fn default_comfy_seed() -> u64 { 42 }
fn default_comfy_dimension() -> u32 { 1024 }

#[derive(Clone, Debug, Serialize)]
pub struct ComfyPromptResponse {
    pub prompt_id: String,
    pub status: String,
}

pub struct AiGeneratorService {
    pub litellm_url: String,
    pub comfyui_url: String,
    client: reqwest::Client,
}

impl AiGeneratorService {
    pub fn new(litellm_url: impl Into<String>, comfyui_url: impl Into<String>) -> Self {
        Self {
            litellm_url: litellm_url.into(),
            comfyui_url: comfyui_url.into(),
            client: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(60))
                .build()
                .unwrap_or_default(),
        }
    }

    /// Generate text/dialogue via LiteLLM endpoint
    pub async fn generate_text(&self, req: &LiteLlmRequest) -> Result<LiteLlmResponse, String> {
        let endpoint = format!("{}/chat/completions", self.litellm_url.trim_end_matches('/'));

        let system_msg = req.system.as_deref().unwrap_or("You are a creative dialogue and narrative writer for non-linear storylets.");

        let body = json!({
            "model": req.model,
            "messages": [
                { "role": "system", "content": system_msg },
                { "role": "user", "content": req.prompt }
            ],
            "temperature": req.temperature,
            "max_tokens": req.max_tokens
        });

        let resp_res = self.client.post(&endpoint).json(&body).send().await;
        match resp_res {
            Ok(resp) if resp.status().is_success() => {
                let data: Value = resp.json().await.map_err(|e| format!("Failed to parse LiteLLM JSON: {e}"))?;
                let text = data.get("choices")
                    .and_then(|c| c.get(0))
                    .and_then(|m| m.get("message"))
                    .and_then(|msg| msg.get("content"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("");

                Ok(LiteLlmResponse {
                    text: text.to_string(),
                    model: req.model.clone(),
                })
            }
            Ok(resp) => {
                Err(format!("LiteLLM returned HTTP {}", resp.status()))
            }
            Err(e) => {
                // If LiteLLM is not running, return informative error
                Err(format!("LiteLLM connection error at {}: {}", self.litellm_url, e))
            }
        }
    }

    /// Submit a text-to-image workflow prompt to ComfyUI
    pub async fn queue_comfy_prompt(&self, req: &ComfyPromptRequest) -> Result<ComfyPromptResponse, String> {
        let endpoint = format!("{}/prompt", self.comfyui_url.trim_end_matches('/'));
        let client_id = uuid::Uuid::new_v4().to_string();

        let prompt_workflow = json!({
            "3": {
                "class_type": "KSampler",
                "inputs": {
                    "cfg": 8,
                    "denoise": 1,
                    "latent_image": ["5", 0],
                    "model": ["4", 0],
                    "negative": ["7", 0],
                    "positive": ["6", 0],
                    "sampler_name": "euler",
                    "scheduler": "normal",
                    "seed": req.seed,
                    "steps": 20
                }
            },
            "4": {
                "class_type": "CheckpointLoaderSimple",
                "inputs": {
                    "ckpt_name": "v1-5-pruned-emaonly.safetensors"
                }
            },
            "5": {
                "class_type": "EmptyLatentImage",
                "inputs": {
                    "batch_size": 1,
                    "height": req.height,
                    "width": req.width
                }
            },
            "6": {
                "class_type": "CLIPTextEncode",
                "inputs": {
                    "clip": ["4", 1],
                    "text": req.prompt
                }
            },
            "7": {
                "class_type": "CLIPTextEncode",
                "inputs": {
                    "clip": ["4", 1],
                    "text": req.negative.as_deref().unwrap_or("ugly, blurry, distorted, low quality, deformed")
                }
            },
            "8": {
                "class_type": "VAEDecode",
                "inputs": {
                    "samples": ["3", 0],
                    "vae": ["4", 2]
                }
            },
            "9": {
                "class_type": "SaveImage",
                "inputs": {
                    "filename_prefix": "DecisionNarrative",
                    "images": ["8", 0]
                }
            }
        });

        let body = json!({
            "client_id": client_id,
            "prompt": prompt_workflow
        });

        let resp_res = self.client.post(&endpoint).json(&body).send().await;
        match resp_res {
            Ok(resp) if resp.status().is_success() => {
                let data: Value = resp.json().await.map_err(|e| format!("Failed to parse ComfyUI JSON: {e}"))?;
                let prompt_id = data.get("prompt_id")
                    .and_then(|v| v.as_str())
                    .unwrap_or(&client_id)
                    .to_string();

                Ok(ComfyPromptResponse {
                    prompt_id,
                    status: "queued".into(),
                })
            }
            Ok(resp) => {
                Err(format!("ComfyUI returned HTTP {}", resp.status()))
            }
            Err(e) => {
                Err(format!("ComfyUI connection error at {}: {}", self.comfyui_url, e))
            }
        }
    }

    /// Get ComfyUI generation history / status for a prompt_id
    pub async fn get_comfy_history(&self, prompt_id: &str) -> Result<Value, String> {
        let endpoint = format!("{}/history/{}", self.comfyui_url.trim_end_matches('/'), prompt_id);
        let resp = self.client.get(&endpoint).send().await.map_err(|e| e.to_string())?;
        if resp.status().is_success() {
            let data: Value = resp.json().await.map_err(|e| e.to_string())?;
            Ok(data)
        } else {
            Err(format!("ComfyUI history returned status {}", resp.status()))
        }
    }

    /// Fetch raw generated image bytes from ComfyUI
    pub async fn get_comfy_image(&self, filename: &str, subfolder: Option<&str>, folder_type: Option<&str>) -> Result<Vec<u8>, String> {
        let mut endpoint = format!("{}/view?filename={}", self.comfyui_url.trim_end_matches('/'), filename);
        if let Some(sub) = subfolder {
            endpoint.push_str(&format!("&subfolder={sub}"));
        }
        if let Some(ftype) = folder_type {
            endpoint.push_str(&format!("&type={ftype}"));
        }

        let resp = self.client.get(&endpoint).send().await.map_err(|e| e.to_string())?;
        if resp.status().is_success() {
            let bytes = resp.bytes().await.map_err(|e| e.to_string())?;
            Ok(bytes.to_vec())
        } else {
            Err(format!("ComfyUI image view returned status {}", resp.status()))
        }
    }
}
