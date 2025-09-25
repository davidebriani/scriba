use clap::Parser;
use cpal::traits::*;
use enigo::{Enigo, Key, Keyboard, Settings};
use std::fs::{create_dir_all, File};
use dirs::config_dir;
use futures_util::StreamExt;
use indicatif::{ProgressBar, ProgressStyle};
use regex::Regex;
use reqwest::Client;
use std::io::Write;
use std::sync::{Arc, Mutex};
use tokio::sync::mpsc;
use tracing::{error, info};
use vosk::{Model, Recognizer};
use dialoguer::Select;
use zip::ZipArchive;
use once_cell::sync::Lazy;
use text2num::{Language, replace_numbers_in_text};

#[derive(Parser)]
#[command(name = "scriba")]
#[command(about = "A real-time speech transcription tool focused on software engineering terms")]
struct Cli {
    /// Sample rate for audio input
    #[arg(short, long, default_value = "16000")]
    sample_rate: u32,
    
    /// Confidence threshold for transcriptions (0.0-1.0)
    #[arg(short, long, default_value = "0.7")]
    confidence_threshold: f64,
    
    /// Show debug output
    #[arg(short, long)]
    debug: bool,
    
    /// Transcription only, no typing
    #[arg(long)]
    no_typing: bool,
    
    /// Force model selection even if model exists
    #[arg(long)]
    select_model: bool,
}

struct TranscriptionResult {
    text: String,
    confidence: f64,
    is_final: bool,
}

struct AudioProcessor {
    recognizer: Arc<Mutex<Recognizer>>,
}

impl AudioProcessor {
    fn new(model: &Model, sample_rate: f32) -> Result<Self, Box<dyn std::error::Error>> {
        let recognizer = Recognizer::new(model, sample_rate)
            .ok_or("Failed to create Vosk recognizer")?;
        
        Ok(AudioProcessor {
            recognizer: Arc::new(Mutex::new(recognizer)),
        })
    }
    
    fn process_audio(&self, audio_data: &[i16]) -> Result<Option<TranscriptionResult>, Box<dyn std::error::Error>> {
        let mut recognizer = self.recognizer.lock().unwrap();
        
        let result = recognizer.accept_waveform(audio_data)?;
        
        match result {
            vosk::DecodingState::Finalized => {
                let complete_result = recognizer.result();
                if let Some(single_result) = complete_result.single() {
                    let text = single_result.text.to_string();
                    if !text.trim().is_empty() {
                        let confidence = single_result.result.first()
                            .map(|word| word.conf as f64)
                            .unwrap_or(0.8);
                        
                        return Ok(Some(TranscriptionResult {
                            text,
                            confidence,
                            is_final: true,
                        }));
                    }
                }
            }
            vosk::DecodingState::Running => {
                let partial_result = recognizer.partial_result();
                let text = partial_result.partial.to_string();
                
                if !text.trim().is_empty() {
                    return Ok(Some(TranscriptionResult {
                        text,
                        confidence: 0.5, // Partial results have lower confidence
                        is_final: false,
                    }));
                }
            }
            _ => {}
        }
        
        Ok(None)
    }
}

struct TextTyper {
    enigo: Enigo,
    last_partial: String,
}

impl TextTyper {
    fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let settings = Settings::default();
        let enigo = Enigo::new(&settings)?;
        
        Ok(TextTyper {
            enigo,
            last_partial: String::new(),
        })
    }
    
    fn type_text(&mut self, result: &TranscriptionResult, confidence_threshold: f64) {
        if result.confidence < confidence_threshold {
            return;
        }
        
        if result.is_final {
            // Clear any partial text that was shown
            if !self.last_partial.is_empty() {
                self.clear_partial_text();
            }
            
            // Type the final result
            let _ = self.enigo.text(&result.text);
            let _ = self.enigo.key(Key::Space, enigo::Direction::Click);
            self.last_partial.clear();
        } else {
            // Handle partial results (optional - can be distracting)
            // For now, we'll skip partial typing to avoid interference
        }
    }
    
    fn clear_partial_text(&mut self) {
        // Clear the partial text by sending backspaces
        for _ in 0..self.last_partial.len() {
            let _ = self.enigo.key(Key::Backspace, enigo::Direction::Click);
        }
    }
}

fn convert_f32_to_i16(input: &[f32]) -> Vec<i16> {
    input.iter().map(|&sample| (sample * 32767.0) as i16).collect()
}

fn setup_audio_stream(sample_rate: u32, tx: mpsc::UnboundedSender<Vec<f32>>) -> Result<(), Box<dyn std::error::Error>> {
    let host = cpal::default_host();
    let device = host
        .default_input_device()
        .ok_or("No input device available")?;
    
    info!("Using input device: {}", device.name()?);
    
    let config = cpal::StreamConfig {
        channels: 1,
        sample_rate: cpal::SampleRate(sample_rate),
        buffer_size: cpal::BufferSize::Default,
    };

    let stream = device.build_input_stream(
        &config,
        move |data: &[f32], _: &cpal::InputCallbackInfo| {
            if let Err(e) = tx.send(data.to_vec()) {
                error!("Failed to send audio data: {}", e);
            }
        },
        |err| {
            error!("Audio stream error: {}", err);
        },
        None,
    )?;
    
    stream.play()?;
    
    // Keep the stream alive
    std::mem::forget(stream);
    
    Ok(())
}

#[derive(Clone)]
struct ModelInfo {
    name: String,
    url: String,
    size: &'static str,
    description: &'static str,
    language: &'static str,
}

static AVAILABLE_MODELS: Lazy<Vec<ModelInfo>> = Lazy::new(|| vec![
    // English Models
    ModelInfo {
        name: "Small English US".to_string(),
        url: "https://alphacephei.com/vosk/models/vosk-model-small-en-us-0.15.zip".to_string(),
        size: "40MB",
        description: "Fast, basic vocabulary",
        language: "English (US)",
    },
    ModelInfo {
        name: "English US (Recommended)".to_string(),
        url: "https://alphacephei.com/vosk/models/vosk-model-en-us-0.22-lgraph.zip".to_string(),
        size: "128MB",
        description: "Better accuracy, larger vocabulary - recommended for developers",
        language: "English (US)",
    },
    ModelInfo {
        name: "Large English US".to_string(),
        url: "https://alphacephei.com/vosk/models/vosk-model-en-us-0.22.zip".to_string(),
        size: "1.8GB",
        description: "Highest accuracy - slow download but best results",
        language: "English (US)",
    },
    ModelInfo {
        name: "English US (GigaSpeech)".to_string(),
        url: "https://alphacephei.com/vosk/models/vosk-model-en-us-0.42-gigaspeech.zip".to_string(),
        size: "2.3GB",
        description: "Latest large model with improved accuracy",
        language: "English (US)",
    },
    ModelInfo {
        name: "English India".to_string(),
        url: "https://alphacephei.com/vosk/models/vosk-model-en-in-0.5.zip".to_string(),
        size: "1GB",
        description: "English model trained on Indian accents",
        language: "English (India)",
    },
    ModelInfo {
        name: "Small English India".to_string(),
        url: "https://alphacephei.com/vosk/models/vosk-model-small-en-in-0.4.zip".to_string(),
        size: "36MB",
        description: "Compact English model for Indian accents",
        language: "English (India)",
    },
    
    // Chinese Models
    ModelInfo {
        name: "Chinese".to_string(),
        url: "https://alphacephei.com/vosk/models/vosk-model-cn-0.22.zip".to_string(),
        size: "1.2GB",
        description: "Standard Chinese model",
        language: "Chinese",
    },
    ModelInfo {
        name: "Small Chinese".to_string(),
        url: "https://alphacephei.com/vosk/models/vosk-model-small-cn-0.22.zip".to_string(),
        size: "42MB",
        description: "Compact Chinese model",
        language: "Chinese",
    },
    
    // Russian Models
    ModelInfo {
        name: "Russian".to_string(),
        url: "https://alphacephei.com/vosk/models/vosk-model-ru-0.42.zip".to_string(),
        size: "2.5GB",
        description: "Large Russian model with high accuracy",
        language: "Russian",
    },
    ModelInfo {
        name: "Small Russian".to_string(),
        url: "https://alphacephei.com/vosk/models/vosk-model-small-ru-0.22.zip".to_string(),
        size: "45MB",
        description: "Compact Russian model",
        language: "Russian",
    },
    
    // French Models
    ModelInfo {
        name: "Small French".to_string(),
        url: "https://alphacephei.com/vosk/models/vosk-model-small-fr-0.22.zip".to_string(),
        size: "41MB",
        description: "Compact French model",
        language: "French",
    },
    
    // German Models
    ModelInfo {
        name: "German".to_string(),
        url: "https://alphacephei.com/vosk/models/vosk-model-de-0.21.zip".to_string(),
        size: "1.2GB",
        description: "Standard German model",
        language: "German",
    },
    ModelInfo {
        name: "Small German".to_string(),
        url: "https://alphacephei.com/vosk/models/vosk-model-small-de-0.15.zip".to_string(),
        size: "45MB",
        description: "Compact German model",
        language: "German",
    },
    
    // Spanish Models
    ModelInfo {
        name: "Spanish".to_string(),
        url: "https://alphacephei.com/vosk/models/vosk-model-es-0.42.zip".to_string(),
        size: "1.4GB",
        description: "Standard Spanish model",
        language: "Spanish",
    },
    ModelInfo {
        name: "Small Spanish".to_string(),
        url: "https://alphacephei.com/vosk/models/vosk-model-small-es-0.42.zip".to_string(),
        size: "39MB",
        description: "Compact Spanish model",
        language: "Spanish",
    },
    
    // Portuguese Models
    ModelInfo {
        name: "Portuguese".to_string(),
        url: "https://alphacephei.com/vosk/models/vosk-model-pt-0.3.zip".to_string(),
        size: "1.2GB",
        description: "Standard Portuguese model",
        language: "Portuguese",
    },
    ModelInfo {
        name: "Small Portuguese".to_string(),
        url: "https://alphacephei.com/vosk/models/vosk-model-small-pt-0.3.zip".to_string(),
        size: "31MB",
        description: "Compact Portuguese model",
        language: "Portuguese",
    },
    
    // Italian Models
    ModelInfo {
        name: "Italian".to_string(),
        url: "https://alphacephei.com/vosk/models/vosk-model-it-0.22.zip".to_string(),
        size: "1.2GB",
        description: "Standard Italian model",
        language: "Italian",
    },
    ModelInfo {
        name: "Small Italian".to_string(),
        url: "https://alphacephei.com/vosk/models/vosk-model-small-it-0.22.zip".to_string(),
        size: "48MB",
        description: "Compact Italian model",
        language: "Italian",
    },
    
    // Dutch Models
    ModelInfo {
        name: "Dutch".to_string(),
        url: "https://alphacephei.com/vosk/models/vosk-model-nl-spraakherkenning-0.6.zip".to_string(),
        size: "860MB",
        description: "Standard Dutch model",
        language: "Dutch",
    },
    ModelInfo {
        name: "Small Dutch".to_string(),
        url: "https://alphacephei.com/vosk/models/vosk-model-small-nl-0.22.zip".to_string(),
        size: "39MB",
        description: "Compact Dutch model",
        language: "Dutch",
    },
    
    // Japanese Models
    ModelInfo {
        name: "Japanese".to_string(),
        url: "https://alphacephei.com/vosk/models/vosk-model-ja-0.22.zip".to_string(),
        size: "1GB",
        description: "Standard Japanese model",
        language: "Japanese",
    },
    ModelInfo {
        name: "Small Japanese".to_string(),
        url: "https://alphacephei.com/vosk/models/vosk-model-small-ja-0.22.zip".to_string(),
        size: "48MB",
        description: "Compact Japanese model",
        language: "Japanese",
    },
    
    // Korean Models
    ModelInfo {
        name: "Small Korean".to_string(),
        url: "https://alphacephei.com/vosk/models/vosk-model-small-ko-0.22.zip".to_string(),
        size: "42MB",
        description: "Compact Korean model",
        language: "Korean",
    },
    
    // Hindi Models
    ModelInfo {
        name: "Hindi".to_string(),
        url: "https://alphacephei.com/vosk/models/vosk-model-hi-0.22.zip".to_string(),
        size: "1.5GB",
        description: "Standard Hindi model",
        language: "Hindi",
    },
    ModelInfo {
        name: "Small Hindi".to_string(),
        url: "https://alphacephei.com/vosk/models/vosk-model-small-hi-0.22.zip".to_string(),
        size: "36MB",
        description: "Compact Hindi model",
        language: "Hindi",
    },
    
    // Ukrainian Models
    ModelInfo {
        name: "Ukrainian".to_string(),
        url: "https://alphacephei.com/vosk/models/vosk-model-uk-v3-lgraph.zip".to_string(),
        size: "350MB",
        description: "Standard Ukrainian model",
        language: "Ukrainian",
    },
    ModelInfo {
        name: "Small Ukrainian".to_string(),
        url: "https://alphacephei.com/vosk/models/vosk-model-small-uk-v3-small.zip".to_string(),
        size: "133MB",
        description: "Compact Ukrainian model",
        language: "Ukrainian",
    },
    
    // Other Languages
    ModelInfo {
        name: "Turkish".to_string(),
        url: "https://alphacephei.com/vosk/models/vosk-model-small-tr-0.3.zip".to_string(),
        size: "35MB",
        description: "Compact Turkish model",
        language: "Turkish",
    },
    ModelInfo {
        name: "Vietnamese".to_string(),
        url: "https://alphacephei.com/vosk/models/vosk-model-small-vn-0.4.zip".to_string(),
        size: "32MB",
        description: "Compact Vietnamese model",
        language: "Vietnamese",
    },
    ModelInfo {
        name: "Arabic".to_string(),
        url: "https://alphacephei.com/vosk/models/vosk-model-ar-mgb2-0.4.zip".to_string(),
        size: "318MB",
        description: "Standard Arabic model",
        language: "Arabic",
    },
    ModelInfo {
        name: "Persian (Farsi)".to_string(),
        url: "https://alphacephei.com/vosk/models/vosk-model-fa-0.5.zip".to_string(),
        size: "1GB",
        description: "Standard Persian model",
        language: "Persian",
    },
    ModelInfo {
        name: "Small Persian (Farsi)".to_string(),
        url: "https://alphacephei.com/vosk/models/vosk-model-small-fa-0.5.zip".to_string(),
        size: "47MB",
        description: "Compact Persian model",
        language: "Persian",
    },
    ModelInfo {
        name: "Small Polish".to_string(),
        url: "https://alphacephei.com/vosk/models/vosk-model-small-pl-0.22.zip".to_string(),
        size: "50MB",
        description: "Compact Polish model",
        language: "Polish",
    },
    ModelInfo {
        name: "Gujarati".to_string(),
        url: "https://alphacephei.com/vosk/models/vosk-model-gu-0.42.zip".to_string(),
        size: "1.4GB",
        description: "Standard Gujarati model",
        language: "Gujarati",
    },
    ModelInfo {
        name: "Small Gujarati".to_string(),
        url: "https://alphacephei.com/vosk/models/vosk-model-small-gu-0.42.zip".to_string(),
        size: "58MB",
        description: "Compact Gujarati model",
        language: "Gujarati",
    },
]);

async fn download_and_extract_model(model: &ModelInfo, dest_dir: &std::path::Path) -> Result<(), Box<dyn std::error::Error>> {
    let zip_path = dest_dir.join("model.zip");
    
    println!("📥 Downloading {} ({})...", model.name, model.size);
    
    let client = Client::new();
    let response = client.get(&model.url).send().await?.error_for_status()?;

    let total_size = response.content_length().ok_or("Failed to get content length")?;
    let mut file = File::create(&zip_path)?;
    let mut stream = response.bytes_stream();

    let pb = ProgressBar::new(total_size);
    pb.set_style(ProgressStyle::default_bar()
        .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {bytes}/{total_bytes} ({eta})")?
        .progress_chars("##-"));

    while let Some(chunk) = stream.next().await {
        let chunk = chunk?;
        file.write_all(&chunk)?;
        pb.inc(chunk.len() as u64);
    }

    pb.finish_with_message("Download complete");
    
    // Extract the zip file
    println!("📦 Extracting model...");
    let file = File::open(&zip_path)?;
    let mut archive = ZipArchive::new(file)?;
    
    for i in 0..archive.len() {
        let mut file = archive.by_index(i)?;
        let outpath = dest_dir.join(file.name());
        
        if file.name().ends_with('/') {
            create_dir_all(&outpath)?;
        } else {
            if let Some(p) = outpath.parent() {
                create_dir_all(p)?;
            }
            let mut outfile = File::create(&outpath)?;
            std::io::copy(&mut file, &mut outfile)?;
        }
    }
    
    // Clean up zip file
    std::fs::remove_file(&zip_path)?;
    
    println!("✅ Model extracted successfully!");
    
    Ok(())
}

fn select_model() -> Result<ModelInfo, Box<dyn std::error::Error>> {
    println!("🎙️  Welcome to Scriba!");
    println!("Please select a speech recognition model:");
    println!();
    
    let items: Vec<String> = AVAILABLE_MODELS.iter()
        .map(|m| format!("{} ({}) - {} ({})", m.name, m.language, m.description, m.size))
        .collect();
    
    let selection = Select::new()
        .with_prompt("Choose a model")
        .items(&items)
        .default(1) // Default to the English US Recommended model
        .interact()?;
    
    Ok(AVAILABLE_MODELS[selection].clone())
}

fn find_model_directory(models_dir: &std::path::Path) -> Option<std::path::PathBuf> {
    if let Ok(entries) = std::fs::read_dir(models_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() && path.file_name().unwrap().to_string_lossy().starts_with("vosk-model") {
                return Some(path);
            }
        }
    }
    None
}

// Enhanced number-to-digit conversion for software engineering contexts
static NUMBER_PATTERNS: Lazy<Vec<(Regex, &'static str)>> = Lazy::new(|| vec![
    // Complex numbers first (more specific patterns)
    (Regex::new(r"\bone thousand\b").unwrap(), "1000"),
    (Regex::new(r"\btwo thousand\b").unwrap(), "2000"),
    (Regex::new(r"\bthree thousand\b").unwrap(), "3000"),
    (Regex::new(r"\bfour thousand\b").unwrap(), "4000"),
    (Regex::new(r"\bfive thousand\b").unwrap(), "5000"),
    (Regex::new(r"\bsix thousand\b").unwrap(), "6000"),
    (Regex::new(r"\bseven thousand\b").unwrap(), "7000"),
    (Regex::new(r"\beight thousand\b").unwrap(), "8000"),
    (Regex::new(r"\bnine thousand\b").unwrap(), "9000"),
    
    // Hundreds
    (Regex::new(r"\bone hundred\b").unwrap(), "100"),
    (Regex::new(r"\btwo hundred\b").unwrap(), "200"),
    (Regex::new(r"\bthree hundred\b").unwrap(), "300"),
    (Regex::new(r"\bfour hundred\b").unwrap(), "400"),
    (Regex::new(r"\bfive hundred\b").unwrap(), "500"),
    (Regex::new(r"\bsix hundred\b").unwrap(), "600"),
    (Regex::new(r"\bseven hundred\b").unwrap(), "700"),
    (Regex::new(r"\beight hundred\b").unwrap(), "800"),
    (Regex::new(r"\bnine hundred\b").unwrap(), "900"),
    
    // Teens
    (Regex::new(r"\beleven\b").unwrap(), "11"),
    (Regex::new(r"\btwelve\b").unwrap(), "12"),
    (Regex::new(r"\bthirteen\b").unwrap(), "13"),
    (Regex::new(r"\bfourteen\b").unwrap(), "14"),
    (Regex::new(r"\bfifteen\b").unwrap(), "15"),
    (Regex::new(r"\bsixteen\b").unwrap(), "16"),
    (Regex::new(r"\bseventeen\b").unwrap(), "17"),
    (Regex::new(r"\beighteen\b").unwrap(), "18"),
    (Regex::new(r"\bnineteen\b").unwrap(), "19"),
    
    // Tens
    (Regex::new(r"\bten\b").unwrap(), "10"),
    (Regex::new(r"\btwenty\b").unwrap(), "20"),
    (Regex::new(r"\bthirty\b").unwrap(), "30"),
    (Regex::new(r"\bforty\b").unwrap(), "40"),
    (Regex::new(r"\bfifty\b").unwrap(), "50"),
    (Regex::new(r"\bsixty\b").unwrap(), "60"),
    (Regex::new(r"\bseventy\b").unwrap(), "70"),
    (Regex::new(r"\beighty\b").unwrap(), "80"),
    (Regex::new(r"\bninety\b").unwrap(), "90"),
    
    // Individual digits (after more complex patterns)
    (Regex::new(r"\bzero\b").unwrap(), "0"),
    (Regex::new(r"\bone\b").unwrap(), "1"),
    (Regex::new(r"\btwo\b").unwrap(), "2"),
    (Regex::new(r"\bthree\b").unwrap(), "3"),
    (Regex::new(r"\bfour\b").unwrap(), "4"),
    (Regex::new(r"\bfive\b").unwrap(), "5"),
    (Regex::new(r"\bsix\b").unwrap(), "6"),
    (Regex::new(r"\bseven\b").unwrap(), "7"),
    (Regex::new(r"\beight\b").unwrap(), "8"),
    (Regex::new(r"\bnine\b").unwrap(), "9"),
    
    // Common programming terms
    (Regex::new(r"\bnull\b").unwrap(), "null"),
    (Regex::new(r"\btrue\b").unwrap(), "true"),
    (Regex::new(r"\bfalse\b").unwrap(), "false"),
    (Regex::new(r"\bempty string\b").unwrap(), "\"\""),
    (Regex::new(r"\bopen paren\b").unwrap(), "("),
    (Regex::new(r"\bclose paren\b").unwrap(), ")"),
    (Regex::new(r"\bopen bracket\b").unwrap(), "["),
    (Regex::new(r"\bclose bracket\b").unwrap(), "]"),
    (Regex::new(r"\bopen brace\b").unwrap(), "{"),
    (Regex::new(r"\bclose brace\b").unwrap(), "}"),
    (Regex::new(r"\bsemicolon\b").unwrap(), ";"),
    (Regex::new(r"\bcolon\b").unwrap(), ":"),
    (Regex::new(r"\bcomma\b").unwrap(), ","),
    (Regex::new(r"\bdot\b").unwrap(), "."),
    (Regex::new(r"\bequals\b").unwrap(), "="),
    (Regex::new(r"\bplus\b").unwrap(), "+"),
    (Regex::new(r"\bminus\b").unwrap(), "-"),
    (Regex::new(r"\btimes\b").unwrap(), "*"),
    (Regex::new(r"\bdivide\b").unwrap(), "/"),
]);

fn convert_words_to_numbers(text: &str) -> String {
    // Use text2num library for comprehensive number conversion
    let en = Language::english();
    // The function directly returns a String, not a Result
    replace_numbers_in_text(text, &en, 0.0)
}

fn enhance_transcription(text: &str) -> String {
    let mut result = text.to_lowercase();
    
    // First, handle complex number conversions
    result = convert_words_to_numbers(&result);
    
    // Then apply simple pattern replacements
    for (pattern, replacement) in NUMBER_PATTERNS.iter() {
        result = pattern.replace_all(&result, *replacement).to_string();
    }
    
    result
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Cli::parse();
    
    // Setup logging
    let log_level = if args.debug { "debug" } else { "info" };
    tracing_subscriber::fmt()
        .with_env_filter(format!("scriba={}", log_level))
        .init();
    
    // Configuration directory in the user's home
    let config_path = config_dir()
        .ok_or("Could not find config directory")?
        .join("scriba");
    create_dir_all(&config_path)?;

    let models_dir = config_path.join("models");
    create_dir_all(&models_dir)?;

    // Check for existing model or prompt for selection
    let model_dir = if args.select_model || find_model_directory(&models_dir).is_none() {
        let selected_model = select_model()?;
        
        let model_specific_dir = models_dir.join(&selected_model.name.replace(" ", "_").to_lowercase());
        
        if !model_specific_dir.exists() || args.select_model {
            create_dir_all(&model_specific_dir)?;
            download_and_extract_model(&selected_model, &model_specific_dir).await?;
        }
        
        // Find the actual model directory inside the downloaded/extracted content
        find_model_directory(&model_specific_dir)
            .ok_or("Could not find extracted model directory")?
    } else {
        find_model_directory(&models_dir)
            .ok_or("Could not find existing model directory")?
    };

    info!("Starting Scriba...");
    info!("Using model: {}", model_dir.display());
    info!("Sample rate: {}", args.sample_rate);
    info!("Confidence threshold: {}", args.confidence_threshold);

    // Load Vosk model
    let model = Model::new(model_dir.to_str().ok_or("Invalid model path")?)
        .ok_or("Failed to load model. Make sure the model exists at the specified path.")?;
    
    // Create audio processing channel
    let (audio_tx, mut audio_rx) = mpsc::unbounded_channel::<Vec<f32>>();
    
    // Setup audio stream
    setup_audio_stream(args.sample_rate, audio_tx)?;
    
    // Create audio processor
    let (result_tx, mut result_rx) = mpsc::unbounded_channel::<TranscriptionResult>();
    let processor = AudioProcessor::new(&model, args.sample_rate as f32)?;
    
    // Spawn audio processing task
    let processor_handle = tokio::spawn(async move {
        let mut buffer = Vec::new();
        const BUFFER_SIZE: usize = 4000; // Process audio in chunks
        
        while let Some(audio_data) = audio_rx.recv().await {
            buffer.extend_from_slice(&audio_data);
            
            if buffer.len() >= BUFFER_SIZE {
                let chunk: Vec<f32> = buffer.drain(..BUFFER_SIZE).collect();
                let i16_chunk = convert_f32_to_i16(&chunk);
                
                match processor.process_audio(&i16_chunk) {
                    Ok(Some(result)) => {
                        if let Err(e) = result_tx.send(result) {
                            error!("Failed to send transcription result: {}", e);
                            break;
                        }
                    }
                    Ok(None) => {}, // No transcription result
                    Err(e) => {
                        error!("Audio processing error: {}", e);
                    }
                }
            }
        }
    });
    
    // Create text typer
    let mut typer = if args.no_typing {
        None
    } else {
        match TextTyper::new() {
            Ok(t) => Some(t),
            Err(e) => {
                error!("Failed to create text typer: {}. Running in no-typing mode.", e);
                None
            }
        }
    };
    
    println!("🎙️  Scriba is running!");
    if typer.is_some() {
        println!("📝 Text will be typed in the currently focused input field.");
        println!("🛑 Press Ctrl+C to stop.");
    } else {
        println!("📄 Typing is disabled. Transcriptions will only be printed.");
        println!("🛑 Press Ctrl+C to stop.");
    }
    println!();
    
    // Process transcription results
    while let Some(result) = result_rx.recv().await {
        if result.is_final && result.confidence >= args.confidence_threshold {
            let enhanced_text = enhance_transcription(&result.text);
            
            info!("📝 Transcription (confidence: {:.2}): {}", result.confidence, enhanced_text);
            
            if let Some(ref mut typer) = typer {
                typer.type_text(&TranscriptionResult {
                    text: enhanced_text,
                    confidence: result.confidence,
                    is_final: result.is_final,
                }, args.confidence_threshold);
            }
        } else if args.debug && !result.is_final {
            info!("🔄 Partial: {}", result.text);
        }
    }

    processor_handle.await?;
    
    Ok(())
}
