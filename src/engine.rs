//! IS-Net preprocessing is adapted from dnanhkhoa/rust-background-removal (MIT).
//! Unlike the upstream CLI, only the inferred mask is resized; original pixels retain
//! their full dimensions and existing alpha. No server or separate UI is included.
use crate::{
    Result, error,
    protocol::{Options, Output},
};
use image::{
    DynamicImage, GrayImage, ImageDecoder, ImageEncoder, ImageReader, RgbaImage,
    imageops::FilterType,
};
use ort::{
    session::{RunOptions, Session, builder::GraphOptimizationLevel},
    tensor::TensorElementType,
    value::{Tensor, ValueType},
};
use std::{
    path::Path,
    sync::atomic::{AtomicBool, Ordering},
    time::Duration,
};

pub const MODEL_SIZE: usize = 1024;
pub const MAX_PIXELS: u64 = 64_000_000;

fn cancelled(cancel: &AtomicBool) -> Result<()> {
    if cancel.load(Ordering::Relaxed) {
        Err(error("Cancelled. No output was saved."))
    } else {
        Ok(())
    }
}

pub fn check_dimensions(width: u32, height: u32) -> Result<()> {
    let pixels = u64::from(width) * u64::from(height);
    if width == 0 || height == 0 || width > 32768 || height > 32768 || pixels > MAX_PIXELS {
        return Err(error(
            "Input must fit within 32,768 pixels per side and 64 megapixels.",
        ));
    }
    Ok(())
}

/// Convert RGB or grayscale PNGs (8/16 bit) to the sRGB input expected by the model.
/// Output is intentionally 8-bit; alpha is preserved independently of ICC conversion.
pub fn read_image(path: &Path) -> Result<RgbaImage> {
    let mut reader = ImageReader::open(path)?.with_guessed_format()?;
    if reader.format() != Some(image::ImageFormat::Png) {
        return Err(error("The host input must be a PNG."));
    }
    let (width, height) = image::image_dimensions(path)?;
    check_dimensions(width, height)?;
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(32768);
    limits.max_image_height = Some(32768);
    limits.max_alloc = Some(512 * 1024 * 1024);
    reader.limits(limits);
    let mut decoder = reader.into_decoder()?;
    let profile = decoder.icc_profile()?;
    let orientation = decoder.orientation()?;
    let mut decoded = DynamicImage::from_decoder(decoder)?;
    if let Some(bytes) = profile {
        let source = moxcms::ColorProfile::new_from_slice(&bytes)?;
        let target = moxcms::ColorProfile::new_srgb();
        let (layout, pixels) = match source.color_space {
            moxcms::DataColorSpace::Rgb => (moxcms::Layout::Rgb, decoded.to_rgb8().into_raw()),
            moxcms::DataColorSpace::Gray => (moxcms::Layout::Gray, decoded.to_luma8().into_raw()),
            _ => return Err(error("Input ICC profile must be RGB or grayscale.")),
        };
        let transform = source.create_transform_8bit(
            layout,
            &target,
            moxcms::Layout::Rgb,
            moxcms::TransformOptions::default(),
        )?;
        let mut converted = vec![0u8; width as usize * height as usize * 3];
        transform.transform(&pixels, &mut converted)?;
        let mut rgba = decoded.to_rgba8();
        for (pixel, rgb) in rgba.pixels_mut().zip(converted.as_chunks::<3>().0) {
            pixel.0[..3].copy_from_slice(rgb);
        }
        decoded = DynamicImage::ImageRgba8(rgba);
    }
    decoded.apply_orientation(orientation);
    Ok(decoded.to_rgba8())
}

/// IMG.LY IS-Net takes NCHW RGB float32: (8-bit channel - 128) / 256.
/// Every packaged variant keeps float32 input/output, including the FP16 weights.
pub fn preprocess(source: &RgbaImage) -> Result<Vec<f32>> {
    check_dimensions(source.width(), source.height())?;
    let resized = image::imageops::resize(
        source,
        MODEL_SIZE as u32,
        MODEL_SIZE as u32,
        FilterType::Triangle,
    );
    let plane = MODEL_SIZE * MODEL_SIZE;
    let mut input = vec![0.0f32; plane * 3];
    for (index, pixel) in resized.pixels().enumerate() {
        for channel in 0..3 {
            input[channel * plane + index] = (f32::from(pixel[channel]) - 128.0) / 256.0;
        }
    }
    Ok(input)
}

pub fn mask_from_tensor(shape: &[i64], values: &[f32]) -> Result<GrayImage> {
    if shape != [1, 1, MODEL_SIZE as i64, MODEL_SIZE as i64]
        || values.len() != MODEL_SIZE * MODEL_SIZE
    {
        return Err(error("The model returned an unexpected mask shape."));
    }
    if values.iter().any(|v| !v.is_finite()) {
        return Err(error("The model returned a non-finite mask."));
    }
    // IS-Net already returns probabilities; applying a second sigmoid damages the cutout.
    let bytes = values
        .iter()
        .map(|v| (v.clamp(0.0, 1.0) * 255.0).round() as u8)
        .collect();
    GrayImage::from_raw(MODEL_SIZE as u32, MODEL_SIZE as u32, bytes)
        .ok_or_else(|| error("Invalid mask buffer."))
}

pub fn compose(
    source: &RgbaImage,
    mask: &GrayImage,
    output: Output,
    cancel: &AtomicBool,
) -> Result<RgbaImage> {
    check_dimensions(source.width(), source.height())?;
    if mask.width() == 0 || mask.height() == 0 {
        return Err(error("The model mask has no pixels."));
    }
    let mask = image::imageops::resize(mask, source.width(), source.height(), FilterType::Triangle);
    let mut result = RgbaImage::new(source.width(), source.height());
    for (index, ((target, original), mask)) in result
        .pixels_mut()
        .zip(source.pixels())
        .zip(mask.pixels())
        .enumerate()
    {
        if index % 65536 == 0 {
            cancelled(cancel)?;
        }
        let foreground = ((u16::from(mask[0]) * u16::from(original[3]) + 127) / 255) as u8;
        match output {
            Output::Foreground => {
                *target = image::Rgba([original[0], original[1], original[2], foreground])
            }
            Output::Background => {
                let alpha = ((u16::from(255 - mask[0]) * u16::from(original[3]) + 127) / 255) as u8;
                *target = image::Rgba([original[0], original[1], original[2], alpha]);
            }
            Output::Mask => *target = image::Rgba([foreground, foreground, foreground, 255]),
        }
    }
    Ok(result)
}

pub fn remove_background(
    input: &Path,
    output: &Path,
    options: &Options,
    runtime: &Path,
    cancel: &AtomicBool,
    progress: &dyn Fn(f32, &str),
) -> Result<()> {
    cancelled(cancel)?;
    if !["isnet", "isnet_fp16", "isnet_quint8"].contains(&options.model.as_str()) {
        return Err(error("Unknown background-removal model."));
    }
    if output.exists() {
        return Err(error("Output already exists; refusing to overwrite it."));
    }
    let dll = runtime.join("bin/onnxruntime.dll");
    let model = runtime
        .join("models")
        .join(format!("{}.onnx", options.model));
    if !dll.is_file() || !model.is_file() {
        return Err(error(
            "A bundled engine or model is missing. Reinstall the Background Removal plugin.",
        ));
    }
    progress(5.0, "Reading image…");
    let source = read_image(input)?;
    cancelled(cancel)?;
    // One request per worker process: this global environment cannot cross plugin jobs.
    // ort rc.10 may panic while resolving a DLL's transitive dependencies. Convert that
    // third-party initialization failure into the same actionable worker error path.
    std::panic::catch_unwind(|| ort::init_from(dll.to_string_lossy()).with_name("PhotoCraft Background Removal").with_telemetry(false).commit())
        .map_err(|_| error("Cannot load the bundled AI engine. Use Windows 10 build 19041 or later, or Windows 11. Reinstall the complete Background Removal plugin to restore its app-local runtime DLLs."))??;
    let tensor = Tensor::from_array((
        [1usize, 3, MODEL_SIZE, MODEL_SIZE],
        preprocess(&source)?.into_boxed_slice(),
    ))?;
    progress(15.0, "Loading local AI model…");
    let threads = std::thread::available_parallelism().map_or(1, |n| n.get().min(4));
    let mut session = Session::builder()?
        .with_log_level(ort::logging::LogLevel::Error)?
        .with_optimization_level(GraphOptimizationLevel::Level1)?
        .with_intra_threads(threads)?
        .with_inter_threads(1)?
        .with_parallel_execution(false)?
        .commit_from_file(model)?;
    let valid_input = session.inputs.len() == 1 && session.inputs.first().is_some_and(|input| matches!(&input.input_type,
        ValueType::Tensor { ty: TensorElementType::Float32, shape, .. } if shape.as_ref() == [1, 3, MODEL_SIZE as i64, MODEL_SIZE as i64]));
    if !valid_input {
        return Err(error("The model input is incompatible with this plugin."));
    }
    cancelled(cancel)?;
    progress(30.0, "Separating subject and background…");
    let run_options = RunOptions::new()?;
    let finished = AtomicBool::new(false);
    let outputs = std::thread::scope(|scope| {
        scope.spawn(|| {
            while !finished.load(Ordering::Acquire) {
                if cancel.load(Ordering::Relaxed) {
                    let _ = run_options.terminate();
                    break;
                }
                std::thread::sleep(Duration::from_millis(50));
            }
        });
        let result = session.run_with_options(ort::inputs![tensor], &run_options);
        finished.store(true, Ordering::Release);
        result
    });
    cancelled(cancel)?;
    let outputs = outputs?;
    let result = outputs
        .values()
        .next()
        .ok_or_else(|| error("The model returned no mask."))?;
    let (shape, probabilities) = result.try_extract_tensor::<f32>()?;
    let mask = mask_from_tensor(shape.as_ref(), probabilities)?;
    progress(85.0, "Restoring the original image size…");
    let result = compose(&source, &mask, options.output, cancel)?;
    cancelled(cancel)?;
    let parent = output
        .parent()
        .ok_or_else(|| error("Missing output directory."))?;
    let temporary = tempfile::NamedTempFile::new_in(parent)?;
    let mut encoder = image::codecs::png::PngEncoder::new(temporary.as_file());
    encoder.set_icc_profile(moxcms::ColorProfile::new_srgb().encode()?)?;
    encoder.write_image(
        result.as_raw(),
        result.width(),
        result.height(),
        image::ExtendedColorType::Rgba8,
    )?;
    cancelled(cancel)?;
    temporary.persist_noclobber(output)?;
    progress(100.0, "Result ready.");
    Ok(())
}
