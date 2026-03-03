# E(X)Hentai Downloader

A **Fast** manga downloader for e-hentai.org and exhentai.org, written in Rust.

## Features

- Multi-threaded downloading for maximum speed
- Automatic retry with verification (up to 5 attempts)
- Supports both e-hentai.org and exhentai.org
- Cross-platform: Linux, macOS, Windows

## Installation

### Download Pre-built Binary

Download the latest release from the [Releases](https://github.com/rniczh/hentai-downloader/releases) page.

| Platform | Download |
|----------|----------|
| Linux (x64) | `hentai-downloader-x86_64-unknown-linux-gnu.tar.gz` |
| macOS (Intel) | `hentai-downloader-x86_64-apple-darwin.tar.gz` |
| macOS (Apple Silicon) | `hentai-downloader-aarch64-apple-darwin.tar.gz` |
| Windows (x64) | `hentai-downloader-x86_64-pc-windows-msvc.zip` |

### Build from Source

Make sure you have [Rust](https://rustup.rs/) installed, then run:

```bash
cargo build --release
```

The executable will be at `target/release/hentai-downloader`.

## Usage

```
hentai-downloader 0.2
Hongsheng Zheng <mathan0203@gmail.com>
Download the Manga from e(x)hentai website.

USAGE:
    hentai-downloader [FLAGS] [OPTIONS] --url <url>

FLAGS:
        --convert-socks5h    将socks5协议的代理地址转换成socks5h协议格式
                             socks5：使用本地DNS解析域名，可能会污染域名，导致不能正常代理
                             socks5h：使用远程DNS解析域名，不会污染域名，能正确代理
    -h, --help               Prints help information
    -r, --retry              强制重试，直到所有下载成功
    -V, --version            Prints version information

OPTIONS:
    -c, --cookie <file>                   The cookie file for access exhentai.org
        --proxy <url>                     自定义代理服务地址，优先级高于proxy-mode参数
                                          只支持：http, https, socks5, socks5h
        --proxy-mode <none|http|socks>    选择不同的代理模式，会自动从环境变量中读取相对应的代理服务地址进行配置
                                          支持的环境变量名称：http_proxy，https_proxy，all_proxy 且支持它们的大写方式
                                          none：使用默认方式，不配置代理服务
                                          http：同时配置Proxy.http，Proxy.https代理服务。会自动读取环境变量(http_proxy|https_proxy)的代理服务地址，变量值不存在就会自动切换到none模式
                                          socks：配置Proxy.all代理服务，会自动读取环境变量(all_proxy)的代理服务地址，变量值不存在就会自动切换到none模式
                                           [default: none]  [possible values: none, http, socks]
    -u, --url <url>                       The url of Manga for which you want to download
```

### Examples

**Download from e-hentai.org:**

```bash
hentai-downloader -u https://e-hentai.org/g/12345/abcdef/
```

**Download from exhentai.org (requires cookie):**

```bash
hentai-downloader -u https://exhentai.org/g/12345/abcdef/ -c cookie.txt
```

Downloaded files will be saved to `tmp{gallery_id}/` directory.

## Cookie Setup (for exhentai.org)

To access exhentai.org, you need to provide your session cookies.

1. Log in to exhentai.org in your browser
2. Open Developer Tools (F12) -> Storage/Application -> Cookies
3. Copy the cookie values and save to `cookie.txt`:

```
ipb_member_id=YOUR_ID; ipb_pass_hash=YOUR_HASH; igneous=YOUR_IGNEOUS
```

## Notice

E(X)Hentai has an implicit image viewing limit per user.
If you exceed this limit, you'll need to wait several hours for it to recover.

## License
GPL-3.0 license
