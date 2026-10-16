use image::{GrayImage, ImageBuffer, Luma, LumaA, Rgba, RgbaImage};
use photocraft_background_removal_plugin::{
    engine,
    protocol::{self, Output},
};
use std::sync::atomic::AtomicBool;

#[test]
fn preprocess_uses_rgb_nchw_and_imgly_normalization() {
    let image = RgbaImage::from_pixel(2, 3, Rgba([0, 128, 255, 0]));
    let tensor = engine::preprocess(&image).unwrap();
    let plane = engine::MODEL_SIZE * engine::MODEL_SIZE;
    assert!(tensor[..plane].iter().all(|v| *v == -0.5));
    assert!(tensor[plane..plane * 2].iter().all(|v| *v == 0.0));
    assert!(tensor[plane * 2..].iter().all(|v| *v == 127.0 / 256.0));
}

#[test]
fn foreground_background_and_mask_preserve_original_dimensions_and_alpha() {
    let source = RgbaImage::from_fn(35, 17, |x, _| {
        Rgba([40, 80, 120, if x == 0 { 0 } else { 128 }])
    });
    let mask = GrayImage::from_pixel(2, 2, Luma([128]));
    let cancel = AtomicBool::new(false);
    for output in [Output::Foreground, Output::Background, Output::Mask] {
        let result = engine::compose(&source, &mask, output, &cancel).unwrap();
        assert_eq!(result.dimensions(), source.dimensions());
        if output == Output::Mask {
            assert_eq!(result.get_pixel(0, 0).0, [0, 0, 0, 255]);
            assert_eq!(result.get_pixel(1, 0).0, [64, 64, 64, 255]);
        } else {
            assert_eq!(result.get_pixel(0, 0).0, [40, 80, 120, 0]);
            assert_eq!(result.get_pixel(1, 0).0, [40, 80, 120, 64]);
        }
    }
}

#[test]
fn mask_is_probability_not_second_sigmoid_and_rejects_nan_or_wrong_shape() {
    let plane = engine::MODEL_SIZE * engine::MODEL_SIZE;
    let mut values = vec![0.0; plane];
    values[..5].copy_from_slice(&[-1.0, 0.0, 0.5, 1.0, 2.0]);
    let shape = [1, 1, 1024, 1024];
    let mask = engine::mask_from_tensor(&shape, &values).unwrap();
    assert_eq!(&mask.as_raw()[..5], &[0, 0, 128, 255, 255]);
    values[20] = f32::NAN;
    assert!(engine::mask_from_tensor(&shape, &values).is_err());
    assert!(engine::mask_from_tensor(&[1, 1024, 1024], &values).is_err());
    assert!(engine::mask_from_tensor(&shape, &[0.0]).is_err());
}

#[test]
fn bounds_and_cancellation_fail_before_output() {
    for size in [(0, 1), (1, 0), (32769, 1), (10000, 10000)] {
        assert!(engine::check_dimensions(size.0, size.1).is_err());
    }
    let image = RgbaImage::new(1, 1);
    assert!(
        engine::compose(
            &image,
            &GrayImage::new(1, 1),
            Output::Foreground,
            &AtomicBool::new(true)
        )
        .is_err()
    );
    assert!(
        engine::compose(
            &image,
            &GrayImage::new(0, 0),
            Output::Foreground,
            &AtomicBool::new(false)
        )
        .is_err()
    );
}

#[test]
fn sixteen_bit_rgba_and_grayscale_are_supported() {
    let temp = tempfile::tempdir().unwrap();
    let rgba: ImageBuffer<Rgba<u16>, Vec<u16>> =
        ImageBuffer::from_pixel(2, 3, Rgba([65535, 32768, 0, 32768]));
    let path = temp.path().join("rgba.png");
    rgba.save(&path).unwrap();
    let decoded = engine::read_image(&path).unwrap();
    assert_eq!(decoded.dimensions(), (2, 3));
    assert_eq!(decoded.get_pixel(0, 0).0, [255, 128, 0, 128]);
    let gray: ImageBuffer<LumaA<u16>, Vec<u16>> = ImageBuffer::from_pixel(3, 2, LumaA([32768, 0]));
    let path = temp.path().join("gray.png");
    gray.save(&path).unwrap();
    assert_eq!(
        engine::read_image(&path).unwrap().get_pixel(0, 0).0,
        [128, 128, 128, 0]
    );
}

#[test]
fn protocol_rejects_unknown_options_paths_and_oversized_requests() {
    let temp = tempfile::tempdir().unwrap();
    let valid = serde_json::json!({"apiVersion":1,"input":temp.path().join("input.png"),"output":temp.path().join("output.png"),"options":{"model":"isnet","output":"foreground"},"outputSize":{"width":25,"height":13},"colorSpace":"srgb"});
    let parse = |value: &serde_json::Value| {
        protocol::parse_job(serde_json::to_vec(value).unwrap().as_slice())
    };
    assert!(parse(&valid).is_ok());
    for (key, value) in [
        ("apiVersion", serde_json::json!(2)),
        ("input", serde_json::json!("relative.png")),
        ("colorSpace", serde_json::json!("display-p3")),
        ("outputSize", serde_json::json!({"width":0,"height":1})),
        ("outputSize", serde_json::json!({"width":32769,"height":1})),
        (
            "outputSize",
            serde_json::json!({"width":10000,"height":10000}),
        ),
        (
            "outputSize",
            serde_json::json!({"width":1,"height":1,"scale":2}),
        ),
        ("unexpected", serde_json::json!(true)),
    ] {
        let mut request = valid.clone();
        request[key] = value;
        assert!(parse(&request).is_err());
    }
    for key in ["outputSize", "colorSpace", "options"] {
        let mut request = valid.clone();
        request.as_object_mut().unwrap().remove(key);
        assert!(parse(&request).is_err());
    }
    for (key, value) in [
        ("output", serde_json::json!("unknown")),
        ("unknown", serde_json::json!(true)),
    ] {
        let mut request = valid.clone();
        request["options"][key] = value;
        assert!(parse(&request).is_err());
    }
    for model in ["../isnet", "other"] {
        let mut request = valid.clone();
        request["options"]["model"] = serde_json::json!(model);
        assert!(parse(&request).is_err());
    }
    let mut request = valid.clone();
    request["output"] = request["input"].clone();
    assert!(parse(&request).is_err());
    std::fs::write(temp.path().join("output.png"), b"existing").unwrap();
    assert!(parse(&valid).is_err());
    assert!(protocol::parse_job(vec![b' '; 16385].as_slice()).is_err());
}

#[test]
fn generic_input_requires_rgba8_and_exact_output_dimensions() {
    let temp = tempfile::tempdir().unwrap();
    let input = temp.path().join("input.png");
    let request = serde_json::json!({"apiVersion":1,"input":input,"output":temp.path().join("output.png"),"options":{"model":"isnet_fp16","output":"foreground"},"outputSize":{"width":3,"height":2},"colorSpace":"srgb"});
    let parse = |value: &serde_json::Value| {
        protocol::parse_job(serde_json::to_vec(value).unwrap().as_slice()).unwrap()
    };
    RgbaImage::from_pixel(3, 2, Rgba([25, 50, 75, 128]))
        .save(&input)
        .unwrap();
    protocol::validate_input(&parse(&request)).unwrap();
    let mut resized = request.clone();
    resized["outputSize"]["width"] = serde_json::json!(6);
    assert!(protocol::validate_input(&parse(&resized)).is_err());
    ImageBuffer::<Rgba<u16>, Vec<u16>>::from_pixel(3, 2, Rgba([65535, 0, 0, 32768]))
        .save(&input)
        .unwrap();
    assert!(protocol::validate_input(&parse(&request)).is_err());
    image::RgbImage::new(3, 2).save(&input).unwrap();
    assert!(protocol::validate_input(&parse(&request)).is_err());
    std::fs::write(&input, b"not a PNG").unwrap();
    assert!(protocol::validate_input(&parse(&request)).is_err());
}
