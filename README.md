# Imagemaker Core (Rust CGI)

Высокопроизводительный CGI-скрипт на **Rust** для динамического изменения размеров изображений и их кэширования "на лету". Это порт оригинального модуля [laravel-imagemaker](https://github.com), полностью переписанный для работы на ультранизких мощностях виртуальных хостингов без накладных расходов на PHP-фреймворки.

## 🎯 Ключевые возможности
- **Три режима ресайза:** `contain` (масштабирование с полями фонового цвета), `cover` (заполнение с обрезкой лишнего по центру) и `stretch` (жесткое растягивание).
- **Наложение вотермарка:** Автоматическое наложение PNG-водяного знака в правый нижний угол с защитой от переполнения на маленьких иконках.
- **Строгая безопасность:** Валидация запрашиваемых размеров по белому списку для защиты от DDoS и спама диска.
- **Энергоэффективность:** Потребляет минимум RAM (< 15 МБ) в момент генерации, а повторные запросы отдаются веб-сервером как чистая статика.

## ⚙️ Настройка конфигурации (config.toml)

Создайте файл `config.toml` в папке, откуда запускается CGI-скрипт (обычно это `cgi-bin`):

```toml
source_dir = "../httpdocs/images"
cache_dir = "../httpdocs/images/icons"
allowed_sizes = ["600x400", "320x240", "120x90"]
allowed_extensions = ["jpg", "jpeg", "png", "gif"]
default_quality = 85
fallback_image = "no-image.png"
resize_mode = "contain"
background_color = "ffffff"
# watermark = "../httpdocs/images/watermark.png"
```

## 🚀 Кросс-компиляция под Linux (для macOS)

Для сборки статически связанного бинарника, который запустится на любом Linux-хостинге (без ошибок glibc), используйте `cargo-zigbuild`:

```bash
# Установка линкера (выполняется один раз)
brew install zig
cargo install cargo-zigbuild
rustup target add x86_64-unknown-linux-musl

# Сборка проекта
cargo zigbuild --release --target x86_64-unknown-linux-musl
```
Исполняемый файл для хостинга будет находиться по пути: `target/x86_64-unknown-linux-musl/release/imagemaker`.

## 🛠 Интеграция с веб-сервером (.htaccess)

Поместите данный код в `.htaccess` в корне вашего сайта для перенаправления запросов на бинарник при отсутствии кэшированного файла:

```apache
Options +ExecCGI
RewriteEngine On

RewriteCond %{REQUEST_FILENAME} !-f
RewriteRule ^images/icons/(.+)\$ /cgi-bin/imagemaker/images/icons/\$1 [L,PT]
```

## Лицензия
MIT
