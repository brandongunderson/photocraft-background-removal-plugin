use crate::{Result, error};
use serde::Deserialize;
use std::{io::Read, path::PathBuf};

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Output {
    Foreground,
    Background,
    Mask,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Options {
    pub model: String,
    pub output: Output,
}

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct OutputSize {
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct Job {
    pub api_version: u32,
    pub input: PathBuf,
    pub output: PathBuf,
    pub options: Options,
    pub output_size: OutputSize,
    pub color_space: String,
}

pub fn parse_job(reader: impl Read) -> Result<Job> {
    let mut bytes = Vec::new();
    reader.take(16_385).read_to_end(&mut bytes)?;
    if bytes.len() > 16_384 {
        return Err(error("Job request exceeds 16 KiB."));
    }
    let job: Job = serde_json::from_slice(&bytes)?;
    if job.api_version != 1
        || job.color_space != "srgb"
        || !job.input.is_absolute()
        || !job.output.is_absolute()
        || job.input == job.output
    {
        return Err(error("Unsupported worker protocol or invalid image paths."));
    }
    crate::engine::check_dimensions(job.output_size.width, job.output_size.height)?;
    if !["isnet", "isnet_fp16", "isnet_quint8"].contains(&job.options.model.as_str()) {
        return Err(error("Unknown background-removal model."));
    }
    if job.output.exists() {
        return Err(error("Output already exists; refusing to overwrite it."));
    }
    Ok(job)
}

/// Generic image jobs must keep background-removal output at the source size.
/// Check the header before loading a model or allocating decoded source pixels.
pub fn validate_input(job: &Job) -> Result<()> {
    use image::{ImageDecoder, ImageReader};
    let reader = ImageReader::open(&job.input)?.with_guessed_format()?;
    if reader.format() != Some(image::ImageFormat::Png) {
        return Err(error("The host input must be an 8-bit RGBA sRGB PNG."));
    }
    let decoder = reader.into_decoder()?;
    let (width, height) = decoder.dimensions();
    crate::engine::check_dimensions(width, height)?;
    if decoder.color_type() != image::ColorType::Rgba8 {
        return Err(error("The host input must be an 8-bit RGBA sRGB PNG."));
    }
    if (width, height) != (job.output_size.width, job.output_size.height) {
        return Err(error(
            "Background Removal keeps the original image size; outputSize must match the input.",
        ));
    }
    Ok(())
}
