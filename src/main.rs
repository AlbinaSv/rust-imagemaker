use image::Rgba;
use image::{GenericImageView, ImageBuffer, RgbaImage};
use serde::Deserialize;
use std::env;
use std::ffi::OsStr;
use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

// Описываем перечисление режимов ресайза
#[derive(Deserialize, Debug, PartialEq)]
#[serde(rename_all = "lowercase")]
enum ResizeMode {
    Contain,
    Cover,
    Stretch,
}

// Структура для хранения разобранного URL-запроса
#[derive(Debug)]
struct ImageRequest {
    size: String,
    target_name: String,
    original_name: String,
    is_real_source: bool, // Флаг: true если оригинал найден, false если используется заглушка
}

// Структура нашего конфигурационного файла config.toml
#[derive(Deserialize, Debug)]
struct AppConfig {
    source_dir: String,
    cache_dir: String,
    allowed_sizes: Vec<String>,
    allowed_extensions: Vec<String>,
    default_quality: u8,
    fallback_image: String,
    resize_mode: ResizeMode,
    background_color: String,
    watermark: Option<String>,
}

// Функция разбора строки размера (например, "320x240")
fn parse_dimensions(size_str: &str) -> Result<(u32, u32), String> {
    let lower_size = size_str.to_lowercase();
    let parts: Vec<&str> = lower_size.split('x').collect();
    if parts.len() != 2 {
        return Err(format!(
            "Invalid size format: '{}'. Expected 'WIDTHxHEIGHT' (e.g., '600x400')",
            size_str
        ));
    }
    let width = parts[0]
        .trim()
        .parse::<u32>()
        .map_err(|_| format!("Invalid width value '{}' in size '{}'", parts[0], size_str))?;
    let height = parts[1]
        .trim()
        .parse::<u32>()
        .map_err(|_| format!("Invalid height value '{}' in size '{}'", parts[1], size_str))?;

    if width == 0 || height == 0 {
        return Err(format!(
            "Width and height must be greater than 0. Got: {}x{}",
            width, height
        ));
    }
    Ok((width, height))
}

// Функция разбора HEX-цвета фона
fn parse_hex_color(hex_str: &str) -> Result<Rgba<u8>, String> {
    let clean_hex = hex_str.trim_start_matches('#');
    if clean_hex.len() != 6 {
        return Err(format!(
            "Invalid HEX color format: '{}'. Expected 6 characters (e.g., 'ffffff')",
            hex_str
        ));
    }
    let r = u8::from_str_radix(&clean_hex[0..2], 16)
        .map_err(|_| format!("Invalid red channel in hex color: {}", hex_str))?;
    let g = u8::from_str_radix(&clean_hex[2..4], 16)
        .map_err(|_| format!("Invalid green channel in hex color: {}", hex_str))?;
    let b = u8::from_str_radix(&clean_hex[4..6], 16)
        .map_err(|_| format!("Invalid blue channel in hex color: {}", hex_str))?;

    Ok(Rgba([r, g, b, 255]))
}

// Функция загрузки и парсинга config.toml
fn load_config() -> Result<AppConfig, String> {
    let config_path = "config.toml";
    if !Path::new(config_path).exists() {
        return Err(format!("Configuration file '{}' not found!", config_path));
    }
    let mut file = fs::File::open(config_path).map_err(|e| e.to_string())?;
    let mut toml_str = String::new();
    file.read_to_string(&mut toml_str)
        .map_err(|e| e.to_string())?;
    let config: AppConfig =
        toml::from_str(&toml_str).map_err(|e| format!("In config.toml syntax error: {}", e))?;
    Ok(config)
}

// Функция получения REQUEST_URI от веб-сервера
fn get_request_uri() -> Result<String, String> {
    if let Ok(uri) = env::var("REQUEST_URI") {
        return Ok(uri);
    }
    if let Ok(uri) = env::var("DOCUMENT_URI") {
        return Ok(uri);
    }
    Err("Environment variable REQUEST_URI or DOCUMENT_URI is not set.".to_string())
}
// Функция разбора и валидации входящего пути запроса
fn parse_request(uri: &str, config: &AppConfig) -> Result<ImageRequest, String> {
    let clean_uri = uri.split('?').next().unwrap_or(uri);
    let segments: Vec<&str> = clean_uri.split('/').filter(|s| !s.is_empty()).collect();

    if segments.len() < 3 {
        return Err("Invalid request path structure".to_string());
    }

    let target_name = segments[segments.len() - 1].to_string();
    let size = segments[segments.len() - 2].to_string();

    if !config.allowed_sizes.contains(&size) {
        return Err(format!("Size '{}' is not allowed by configuration", size));
    }

    let path_obj = Path::new(&target_name);
    if path_obj.extension().and_then(OsStr::to_str) != Some("webp") {
        return Err("Only .webp thumbnail generation is supported".to_string());
    }

    let file_stem = path_obj
        .file_stem()
        .and_then(OsStr::to_str)
        .ok_or("Cannot extract filename stem")?;

    let mut original_name = String::new();
    let mut found = false;

    // Сканируем оригинальные расширения файлов
    for ext in &config.allowed_extensions {
        let test_name = format!("{}.{}", file_stem, ext);
        let full_source_path = Path::new(&config.source_dir).join(&test_name);
        if full_source_path.exists() {
            original_name = test_name;
            found = true;
            break;
        }
    }

    // КРИТИЧЕСКИЙ ФИКС: Если оригинал не найден, берём заглушку, но запоминаем, что found = false
    if !found {
        let fallback_path = Path::new(&config.source_dir).join(&config.fallback_image);
        if fallback_path.exists() {
            original_name = config.fallback_image.clone();
        } else {
            return Err("Original image and fallback image not found".to_string());
        }
    }

    Ok(ImageRequest {
        size,
        target_name,
        original_name,
        is_real_source: found, // Фиксируем факт нахождения оригинала
    })
}

// Функция обработки изображения и наложения вотермарка
fn generate_thumbnail(
    source_path: &Path,
    target_path: &Path,
    target_width: u32,
    target_height: u32,
    config: &AppConfig,
    bg_color: image::Rgba<u8>,
    should_cache: bool, // Флаг: сохранять ли файл физически на диск
) -> Result<image::RgbaImage, String> {
    let img = image::open(source_path)
        .map_err(|e| format!("Failed to open source image '{:?}': {}", source_path, e))?;

    let mut canvas: image::RgbaImage;

    // Выбираем режим ресайза
    match config.resize_mode {
        ResizeMode::Contain => {
            canvas = ImageBuffer::from_fn(target_width, target_height, |_, _| {
                image::Rgba([bg_color.0[0], bg_color.0[1], bg_color.0[2], 255])
            });
            let resized = img.resize(
                target_width,
                target_height,
                image::imageops::FilterType::Lanczos3,
            );
            let (r_width, r_height) = resized.dimensions();
            let x_offset = (target_width - r_width) / 2;
            let y_offset = (target_height - r_height) / 2;

            image::imageops::overlay(
                &mut canvas,
                &resized.to_rgba8(),
                x_offset as i64,
                y_offset as i64,
            );
        }
        ResizeMode::Cover => {
            let resized = img.resize_to_fill(
                target_width,
                target_height,
                image::imageops::FilterType::Lanczos3,
            );
            canvas = resized.to_rgba8();
        }
        ResizeMode::Stretch => {
            let resized = img.resize_exact(
                target_width,
                target_height,
                image::imageops::FilterType::Lanczos3,
            );
            canvas = resized.to_rgba8();
        }
    }

    // Накладываем вотермарк (если он включен)
    if let Some(ref watermark_path_str) = config.watermark {
        let watermark_path = Path::new(watermark_path_str);
        if watermark_path.exists() {
            let watermark_img = image::open(watermark_path).map_err(|e| {
                format!(
                    "Failed to open watermark image '{:?}': {}",
                    watermark_path, e
                )
            })?;
            let (wm_width, wm_height) = watermark_img.dimensions();
            let padding = 10;
            let wm_x = if target_width > wm_width + padding {
                target_width - wm_width - padding
            } else {
                0
            };
            let wm_y = if target_height > wm_height + padding {
                target_height - wm_height - padding
            } else {
                0
            };

            image::imageops::overlay(
                &mut canvas,
                &watermark_img.to_rgba8(),
                wm_x as i64,
                wm_y as i64,
            );
        }
    }

    // КРИТИЧЕСКИЙ ФИКС: Сохраняем на диск только если это реальный оригинал товара!
    if should_cache {
        if let Some(parent) = target_path.parent() {
            fs::create_dir_all(parent)
                .map_err(|e| format!("Failed to create directories: {}", e))?;
        }
        canvas
            .save_with_format(target_path, image::ImageFormat::WebP)
            .map_err(|e| format!("Failed to save WebP thumbnail: {}", e))?;
    }

    Ok(canvas)
}
// Главная точка входа CGI-скрипта
fn main() {
    let uri = match get_request_uri() {
        Ok(u) => u,
        Err(err) => {
            println!("Status: 500 Internal Server Error");
            println!("Content-Type: text/plain; charset=utf-8\n");
            println!("X CGI Environment Error: {}", err);
            return;
        }
    };

    let config = match load_config() {
        Ok(cfg) => cfg,
        Err(err) => {
            println!("Status: 500 Internal Server Error");
            println!("Content-Type: text/plain; charset=utf-8\n");
            println!("X Config Error: {}", err);
            return;
        }
    };

    let request = match parse_request(&uri, &config) {
        Ok(req) => req,
        Err(err) => {
            println!("Status: 403 Forbidden");
            println!("Content-Type: text/plain; charset=utf-8\n");
            println!("X Access Denied: {}", err);
            return;
        }
    };

    let (width, height) = match parse_dimensions(&request.size) {
        Ok(dims) => dims,
        Err(err) => {
            println!("Status: 400 Bad Request");
            println!("Content-Type: text/plain; charset=utf-8\n");
            println!("X Invalid Dimensions: {}", err);
            return;
        }
    };

    let color = match parse_hex_color(&config.background_color) {
        Ok(c) => c,
        Err(err) => {
            println!("Status: 500 Internal Server Error");
            println!("Content-Type: text/plain; charset=utf-8\n");
            println!("X Color Config Error: {}", err);
            return;
        }
    };

    // Строим пути к файлам для обработки
    let full_source_path = Path::new(&config.source_dir).join(&request.original_name);
    let final_image_path = Path::new(&config.cache_dir)
        .join(&request.size)
        .join(&request.target_name);

    // Запускаем безопасную генерацию. Флаг request.is_real_source управляет записью на диск
    let canvas = match generate_thumbnail(
        &full_source_path,
        &final_image_path,
        width,
        height,
        &config,
        color,
        request.is_real_source, // Если false (заглушка) — на диск ничего не запишется!
    ) {
        Ok(img) => img,
        Err(err) => {
            println!("Status: 500 Internal Server Error");
            println!("Content-Type: text/plain; charset=utf-8\n");
            println!("X Image Processing Error: {}", err);
            return;
        }
    };

    // ОТВЕТ ВЕБ-СЕРВЕРУ (CGI): Отдаем картинку из оперативной памяти в браузер
    println!("Status: 200 OK");
    println!("Content-Type: image/webp\n");

    // Используем Курсор в качестве буфера в памяти, который поддерживает Seek
    let mut buffer = std::io::Cursor::new(Vec::new());

    // Кодируем WebP в буфер памяти
    if canvas
        .write_to(&mut buffer, image::ImageFormat::WebP)
        .is_ok()
    {
        let mut stdout = std::io::stdout();
        // Сбрасываем готовые байты напрямую в stdout веб-сервера
        let _ = stdout.write_all(buffer.into_inner().as_slice());
        let _ = stdout.flush();
    }
}
