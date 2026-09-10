# rust-imagemaker

![Rust](https://shields.io/badge/rust-2021-orange.svg)
![License](https://shields.io/badge/license-MIT-blue.svg)
![Platform](https://shields.io/platform-Linux%20%7C%20Windows%20%7C%20macOS-lightgrey.svg)

A universal, high-performance CGI image resizer written in Rust, built for web sites and apps. It acts as an autonomous on-the-fly image processing and caching microservice that can be plugged into *any* backend or CMS without modifying your website's codebase.

[Читать на русском языке](./README.ru.md)

## 💡 Motivation & Problem Solving

Every web developer faces a problem when a website redesign completely transforms the UI, changing the dimensions and aspect ratios of thumbnails or introducing entirely new image variants. This is especially traumatic if you manage an e-commerce platform with thousands of products and suddenly need to recreate every single product thumbnail all at once.

This Rust-based CGI script helps solve this exact infrastructure problem quickly, effectively, once and for all.

### How it works in practice:
Imagine all your original high-resolution product photos are stored safely in the `/images` folder, and your database only holds the raw filename for each item (e.g., `hash12345.jpg`).

You simply point your HTML `<img>` tag to the desired target dimensions:
```html
<img src="/images/icons/320x240/hash12345.webp" alt="Product Image">
```

After a quick configuration setup, the magic happens behind the scenes:
1. The web server attempts to fetch the file at `/images/icons/320x240/hash12345.webp`.
2. If the cached file does not exist yet, `.htaccess` reroutes the request directly to our Rust binary.
3. 🛡️ **DDoS & Disk Spam Protection:** The script never generates arbitrary image dimensions passed from the outside. It processes **only strictly defined dimensions specified in `config.toml`** (whitelist). If an unauthorized size is requested in the browser address bar, the script rejects the request immediately.
4. The script locates the source photo at `/images/hash12345.jpg`, processes it into a permitted `320x240` format, overlays a watermark, and writes it directly to the cache folder as `/images/icons/320x240/hash12345.webp`.
5. For all subsequent visits, the web server serves it immediately as pure static content, without loading the CPU or executing Rust again.

If your design changes down the road, you don't need to run heavy backend migration scripts. **Just delete the cache directory (`/icons`) completely along with all its contents**, adjust your allowed sizes in the configuration file, and update your front-end layout paths. The new thumbnail cache will gracefully rebuild itself on-the-fly as users visit the pages.

## 🚀 Quick Start: Using Pre-compiled Binaries

For those who are not familiar with code compilation or do not want to build the project from source manually, you can use pre-compiled standalone binaries for different operating systems.

You only need to follow 4 simple steps:

### Step 1. Download the required file
Go to the **Releases** section on the right side of this repository and choose one of the three ready-made options for your system:
* `rust-imagemaker-linux-x86_64` — for running on servers under Linux-like OS (used on 99% of regular shared hostings, such as Mchost, Timeweb, etc.).
* `rust-imagemaker-windows-x86_64.exe` — for working on servers running Windows Server (IIS).
* `rust-imagemaker-macos-arm64` — for local testing and development on Mac computers (Apple Silicon).

### Step 2. Placement and mandatory renaming
Place the downloaded file into the server directory from which your hosting provider allows executing CGI scripts (most often this folder is named `cgi-bin`).

**Important:** After uploading, you must rename the file:
* On Linux and macOS, rename the file to: **`rust-imagemaker`** (without any extensions at the end).
* On Windows Server, rename the file to: **`rust-imagemaker.exe`**

### Step 3. Set file permissions (for Linux / macOS)
Via an FTP client (e.g., FileZilla) or your hosting file manager, make sure to set the file permissions for `rust-imagemaker` to **`755`** (executable permission). Without this, the server will return a *500 Internal Server Error*.

### Step 4. Configuration and server setup
1. Copy the configuration template from the `examples/config.toml.example` directory into the folder where your binary lives, rename it to **`config.toml`**, and adjust your paths.
2. Copy the rewrite rules from `examples/.htaccess.example` into your main **`.htaccess`** file in the website root directory.

## 📁 Path Customization & Templates
The project is designed to be as flexible as possible. You can customize the directory structure to fit any needs:
* 📂 **Complete freedom of cache placement:** The folder for saving ready-made previews (`icons`) **does not have to be inside the folder with original images**. It can be located anywhere in your server's file structure. The main thing is to specify the exact relative or absolute path to it in the `cache_dir` variable inside `config.toml`.
* 🖼️ **Fallback image placement:** The `fallback_image = "no-image.png"` parameter sets the filename of a fallback image that will be automatically shown to the user if the original product file is missing. This file **must be located in the same folder where all your original source images are stored** (`source_dir`).

Ready-made configuration templates are available in the repository:
* `examples/config.toml.example` — a detailed configuration file with comments for each line (paths, allowed dimensions whitelist, resize modes `contain`/`cover`/`stretch`, and watermark configuration).
* `examples/.htaccess.example` — ready-made URL rewrite rules for the Apache web server.

## License
This project is open-source software licensed under the [MIT License](./LICENSE).
