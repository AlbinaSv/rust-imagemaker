use image::Rgba;
use image::{GenericImageView, ImageBuffer, RgbaImage};
use serde::Deserialize;
use std::env;
use std::ffi::OsStr;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};

// Описываем перечисление режимов ресайза
#[derive(Deserialize, Debug, PartialEq)]
#[serde(rename_all = "lowercase")]
enum ResizeMode {
    Contain,
    Cover,
    Stretch,
}

#[derive(Debug)]
struct ImageRequest {
    size: String,          // Например: "600x400"
    target_name: String,   // Например: "product-1.webp"
    original_name: String, // Например: "product-1.jpg" (или png, в зависимости от исходника)
}

// Описываем структуру нашего конфигурационного файла
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

fn parse_dimensions(size_str: &str) -> Result<(u32, u32), String> {
    // 1. Создаем полноценную переменную. Теперь она будет жить до конца функции!
    let lower_size = size_str.to_lowercase();

    // 2. Разбиваем её на части
    let parts: Vec<&str> = lower_size.split('x').collect();

    // Нам строго нужно ровно 2 части (ширина и высота)
    if parts.len() != 2 {
        return Err(format!(
            "Invalid size format: '{}'. Expected 'WIDTHxHEIGHT' (e.g., '600x400')",
            size_str
        ));
    }

    // Парсим ширину в u32
    let width = parts[0]
        .trim()
        .parse::<u32>()
        .map_err(|_| format!("Invalid width value '{}' in size '{}'", parts[0], size_str))?;

    // Парсим высоту в u32
    let height = parts[1]
        .trim()
        .parse::<u32>()
        .map_err(|_| format!("Invalid height value '{}' in size '{}'", parts[1], size_str))?;

    // Защита от нулевых размеров
    if width == 0 || height == 0 {
        return Err(format!(
            "Width and height must be greater than 0. Got: {}x{}",
            width, height
        ));
    }

    // Возвращаем кортеж из двух чисел
    Ok((width, height))
}

fn parse_hex_color(hex_str: &str) -> Result<Rgba<u8>, String> {
    // Убираем возможный знак #, если пользователь случайно ввел его в конфиге
    let clean_hex = hex_str.trim_start_matches('#');

    // HEX-цвет должен состоять строго из 6 символов (например, ffffff или 33aaff)
    if clean_hex.len() != 6 {
        return Err(format!(
            "Invalid HEX color format: '{}'. Expected 6 characters (e.g., 'ffffff')",
            hex_str
        ));
    }

    // Парсим каждые 2 символа из шестнадцатеричной системы (radix 16) в число u8
    let r = u8::from_str_radix(&clean_hex[0..2], 16)
        .map_err(|_| format!("Invalid red channel in hex color: {}", hex_str))?;
    let g = u8::from_str_radix(&clean_hex[2..4], 16)
        .map_err(|_| format!("Invalid green channel in hex color: {}", hex_str))?;
    let b = u8::from_str_radix(&clean_hex[4..6], 16)
        .map_err(|_| format!("Invalid blue channel in hex color: {}", hex_str))?;

    // Возвращаем готовый RGB-пиксель для библиотеки image
    Ok(Rgba([r, g, b, 255]))
}

fn ensure_cache_dir(config: &AppConfig, size: &str) -> Result<PathBuf, String> {
    // 1. Строим полный путь к подпапке размера: например, "./public/images/icons/600x400"
    let target_dir = Path::new(&config.cache_dir).join(size);

    // 2. Проверяем, существует ли папка. Если нет — создаем всю цепочку
    if !target_dir.exists() {
        fs::create_dir_all(&target_dir)
            .map_err(|e| format!("Failed to create cache directory '{:?}': {}", target_dir, e))?;
    }

    // Возвращаем готовый путь в формате PathBuf (удобный тип для путей в Rust)
    Ok(target_dir)
}

// Функция для чтения и парсинга config.toml
fn load_config() -> Result<AppConfig, String> {
    let config_path = "config.toml";

    // Проверяем, существует ли файл физически
    if !Path::new(config_path).exists() {
        return Err(format!("Configuration file '{}' not found!", config_path));
    }

    // Открываем и читаем файл в строку
    let mut file = fs::File::open(config_path).map_err(|e| e.to_string())?;
    let mut toml_str = String::new();
    file.read_to_string(&mut toml_str)
        .map_err(|e| e.to_string())?;

    // Десериализуем (парсим) строку TOML в нашу структуру AppConfig
    let config: AppConfig =
        toml::from_str(&toml_str).map_err(|e| format!("In config.toml syntax error: {}", e))?;

    Ok(config)
}

fn get_request_uri() -> Result<String, String> {
    // 1. Сначала пытаемся получить стандартный REQUEST_URI (работает в Apache и Nginx + fcgiwrap)
    if let Ok(uri) = env::var("REQUEST_URI") {
        return Ok(uri);
    }

    // 2. Если пустой, пробуем DOCUMENT_URI (часто используется в специфичных конфигах Nginx)
    if let Ok(uri) = env::var("DOCUMENT_URI") {
        return Ok(uri);
    }

    // 3. Если скрипт запущен вручную в консоли (без сервера), возвращаем ошибку
    Err("Environment variable REQUEST_URI or DOCUMENT_URI is not set. Are you running this script inside a web server?".to_string())
}

fn parse_request(uri: &str, config: &AppConfig) -> Result<ImageRequest, String> {
    // 1. Очищаем URI от возможных query-параметров (например, /path?v=123)
    let clean_uri = uri.split('?').next().unwrap_or(uri);

    // 2. Разбиваем путь на сегменты по слэшу и убираем пустые элементы
    let segments: Vec<&str> = clean_uri.split('/').filter(|s| !s.is_empty()).collect();

    // Ожидаемая структура пути в конце: [... , "icons", "600x400", "product-1.webp"]
    // Значит, нам нужно как минимум 3 сегмента
    if segments.len() < 3 {
        return Err("Invalid request path structure".to_string());
    }

    // Забираем элементы с конца массива
    let target_name = segments[segments.len() - 1].to_string();
    let size = segments[segments.len() - 2].to_string();

    // 3. СТРОГАЯ ВАЛИДАЦИЯ №1: Проверяем размер по белому списку из config.toml
    if !config.allowed_sizes.contains(&size) {
        return Err(format!("Size '{}' is not allowed by configuration", size));
    }

    // 4. СТРОГАЯ ВАЛИДАЦИЯ №2: Проверяем, что запрашивают именно .webp (наш кэш)
    let path_obj = Path::new(&target_name);
    if path_obj.extension().and_then(OsStr::to_str) != Some("webp") {
        return Err("Only .webp thumbnail generation is supported".to_string());
    }

    // 5. Ищем оригинальное имя файла
    // Так как в кэше имя "file.webp", оригинал на диске может быть "file.jpg", "file.png" и т.д.
    let file_stem = path_obj
        .file_stem()
        .and_then(OsStr::to_str)
        .ok_or("Cannot extract filename stem")?;

    let mut original_name = String::new();
    let mut found = false;

    // Сканируем допустимые расширения из конфига и проверяем физическое наличие файла оригинала
    for ext in &config.allowed_extensions {
        let test_name = format!("{}.{}", file_stem, ext);
        let full_source_path = Path::new(&config.source_dir).join(&test_name);

        if full_source_path.exists() {
            original_name = test_name;
            found = true;
            break;
        }
    }

    // Если оригинальный файл не найден среди разрешенных расширений, используем заглушку
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
    })
}

fn generate_thumbnail(
    source_path: &Path,
    target_path: &Path,
    target_width: u32,
    target_height: u32,
    config: &AppConfig,
    bg_color: image::Rgba<u8>,
) -> Result<(), String> {
    // 1. Открываем оригинальное изображение с диска
    let img = image::open(source_path)
        .map_err(|e| format!("Failed to open source image '{:?}': {}", source_path, e))?;

    // 2. Инициализируем результирующий холст/картинку
    let mut canvas: image::RgbaImage;

    // 3. Логика обработки в зависимости от выбранного режима в конфиге
    match config.resize_mode {
        // Режим CONTAIN: Пропорционально сжимаем, центрируем и заливаем пустые поля фоном
        ResizeMode::Contain => {
            // Создаем пустой холст нужного размера, заполненный цветом фона (например, белым)
            canvas = ImageBuffer::from_fn(target_width, target_height, |_, _| {
                image::Rgba([bg_color.0[0], bg_color.0[1], bg_color.0[2], 255])
            });

            // resize пропорционально сжимает картинку, чтобы она влезла в рамки холста
            let resized = img.resize(
                target_width,
                target_height,
                image::imageops::FilterType::Lanczos3,
            );
            let (r_width, r_height) = resized.dimensions();

            // Вычисляем координаты центра, чтобы отцентрировать картинку на холсте
            let x_offset = (target_width - r_width) / 2;
            let y_offset = (target_height - r_height) / 2;

            // Накладываем измененную картинку на подготовленный холст
            image::imageops::overlay(
                &mut canvas,
                &resized.to_rgba8(),
                x_offset as i64,
                y_offset as i64,
            );
        }

        // Режим COVER: Картинка заполняет холст целиком, лишнее обрезается по центру
        ResizeMode::Cover => {
            // resize_to_fill масштабирует по меньшей стороне и автоматически
            // обрезает (crop) изображение по центру до указанных размеров
            let resized = img.resize_to_fill(
                target_width,
                target_height,
                image::imageops::FilterType::Lanczos3,
            );
            canvas = resized.to_rgba8();
        }

        // Режим STRETCH: Изображение жестко растягивается под размеры, искажая пропорции
        ResizeMode::Stretch => {
            // resize_exact игнорирует соотношение сторон исходника
            let resized = img.resize_exact(
                target_width,
                target_height,
                image::imageops::FilterType::Lanczos3,
            );
            canvas = resized.to_rgba8();
        }
    }

    // 4. НАЛОЖЕНИЕ ВОТЕРМАРКА (в правый нижний угол)
    if let Some(ref watermark_path_str) = config.watermark {
        let watermark_path = Path::new(watermark_path_str);

        // Проверяем, существует ли файл вотермарка на диске
        if watermark_path.exists() {
            let watermark_img = image::open(watermark_path).map_err(|e| {
                format!(
                    "Failed to open watermark image '{:?}': {}",
                    watermark_path, e
                )
            })?;

            let (wm_width, wm_height) = watermark_img.dimensions();

            // Задаем безопасный отступ от правого и нижнего края (например, 10 пикселей)
            let padding = 10;

            // Вычисляем координаты для правого нижнего угла.
            // Если вотермарк вдруг оказался больше самого результирующего холста,
            // используем координаты 0,0 (защита от отрицательного переполнения)
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

            // Накладываем вотермарк поверх сгенерированного холста
            image::imageops::overlay(
                &mut canvas,
                &watermark_img.to_rgba8(),
                wm_x as i64,
                wm_y as i64,
            );
        }
    }

    // 5. Сохраняем готовый результат на диск в формате WebP
    canvas
        .save_with_format(target_path, image::ImageFormat::WebP)
        .map_err(|e| format!("Failed to save WebP thumbnail: {}", e))?;

    Ok(())
}

fn main() {
    // 1. Пробуем получить URI из переменной окружения сервера
    let uri = match get_request_uri() {
        Ok(u) => u,
        Err(err) => {
            // Если запустили в консоли вручную без сервера — выдаем ошибку для отладки
            println!("Status: 500 Internal Server Error");
            println!("Content-Type: text/plain; charset=utf-8");
            println!();
            println!("❌ CGI Environment Error: {}", err);
            return;
        }
    };

    // 2. Загружаем конфигурацию TOML
    let config = match load_config() {
        Ok(cfg) => cfg,
        Err(err) => {
            println!("Status: 500 Internal Server Error");
            println!("Content-Type: text/plain; charset=utf-8");
            println!();
            println!("❌ Config Error: {}", err);
            return;
        }
    };

    // 3. Парсим и валидируем входящий путь
    let request = match parse_request(&uri, &config) {
        Ok(req) => req,
        Err(err) => {
            // Если запрос не прошел валидацию (не тот размер, расширение и т.д.)
            println!("Status: 403 Forbidden");
            println!("Content-Type: text/plain; charset=utf-8");
            println!();
            println!("❌ Access Denied: {}", err);
            return;
        }
    };

    // 4. Автоматически проверяем и создаем папку для кэша
    let cache_dir_path = match ensure_cache_dir(&config, &request.size) {
        Ok(path) => path,
        Err(err) => {
            println!("Status: 500 Internal Server Error");
            println!("Content-Type: text/plain; charset=utf-8");
            println!();
            println!("❌ Filesystem Error: {}", err);
            return;
        }
    };

    // Строим пути к файлам
    let full_source_path = Path::new(&config.source_dir).join(&request.original_name);
    let final_image_path = cache_dir_path.join(&request.target_name);

    // let color = parse_hex_color(&config.background_color);

    // Разбираем строку размера (например, "600x400") на width и height
    let (width, height) = match parse_dimensions(&request.size) {
        Ok(dims) => dims,
        Err(err) => {
            println!("Status: 400 Bad Request");
            println!("Content-Type: text/plain; charset=utf-8");
            println!();
            println!("❌ Invalid Dimensions: {}", err);
            return;
        }
    };

    // Парсим HEX-цвет (раскомментировали и добавили обработку ошибок)
    let color = match parse_hex_color(&config.background_color) {
        Ok(c) => c,
        Err(err) => {
            println!("Status: 500 Internal Server Error");
            println!("Content-Type: text/plain; charset=utf-8");
            println!();
            println!("❌ Color Config Error: {}", err);
            return;
        }
    };

    // 5. Запускаем генерацию графики
    if let Err(err) = generate_thumbnail(
        &full_source_path,
        &final_image_path,
        width,
        height,
        &config,
        color,
    ) {
        println!("Status: 500 Internal Server Error");
        println!("Content-Type: text/plain; charset=utf-8");
        println!();
        println!("❌ Image Processing Error: {}", err);
        return;
    }

    // 6. ОТВЕТ ВЕБ-СЕРВЕРУ (CGI): Выдаем правильные HTTP-заголовки и бинарные данные файла
    println!("Status: 200 OK");
    println!("Content-Type: image/webp");
    println!(); // Пустая строка отделяет заголовки от тела ответа

    // Читаем только что созданный WebP файл и выводим его в стандартный поток (stdout)
    if let Ok(mut file) = fs::File::open(&final_image_path) {
        let mut buffer = Vec::new();
        if file.read_to_end(&mut buffer).is_ok() {
            // Записываем бинарные данные картинки прямо в консоль ответа сервера
            use std::io::Write;
            let _ = std::io::stdout().write_all(&buffer);
        }
    }
}
