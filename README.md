# Stremio Discord RPC (macOS)

Stremio'da izlediğin içeriği Discord profilinde göster. macOS için yazılmış, Stremio v5 ile çalışır.

![Rust](https://img.shields.io/badge/Rust-000000?logo=rust&logoColor=white)
![macOS](https://img.shields.io/badge/macOS-000000?logo=apple&logoColor=white)
![Discord](https://img.shields.io/badge/Discord_RPC-5865F2?logo=discord&logoColor=white)

## Özellikler

- **Otomatik algılama** — Stremio'da ne izlediğini otomatik tespit eder
- **Poster görseli** — Film/dizi posteri büyük görsel olarak, Stremio ikonu küçük görsel olarak
- **Yuvarlatılmış köşeler** — Poster görselleri yuvarlatılmış köşelerle gösterilir
- **Bölüm bilgisi** — Dizi izlerken bölüm adı, sezon ve bölüm numarası gösterilir
- **Progress bar** — Geçen süre ve toplam süre ile ilerleme çubuğu
- **Pause tespiti** — Oynatma duraklatıldığında "Paused", devam ederken "Playing"
- **Çoklu kaynak desteği** — Cinemeta (IMDB), Kitsu (anime), Animecix metadata API'leri

## Nasıl Çalışır

Stremio v5 macOS uygulaması WKWebView kullanır ve izleme durumunu localStorage'da (SQLite) tutar. Bu uygulama:

1. Stremio'nun localStorage veritabanını okur (`~/Library/WebKit/com.westbridge.stremio5-mac/...`)
2. `library_recent` tablosundaki `_mtime` ile aktif izlemeyi tespit eder
3. Cinemeta/Kitsu/Animecix API'lerinden bölüm bilgisi ve poster çeker
4. Discord IPC üzerinden Rich Presence olarak gösterir

## Kurulum

### Gereksinimler

- macOS
- [Rust](https://rustup.rs/)
- Stremio v5 masaüstü uygulaması
- Discord masaüstü uygulaması

### Discord Uygulaması Oluşturma

1. [Discord Developer Portal](https://discord.com/developers/applications) adresine git
2. **New Application** tıkla, adını **Stremio** koy
3. Application ID'yi kopyala

### Build

```bash
git clone https://github.com/phirios/stremio-discord-rpc.git
cd stremio-discord-rpc
```

`src/main.rs` dosyasında `DISCORD_APP_ID` sabitini kendi Application ID'nle değiştir:

```rust
const DISCORD_APP_ID: &str = "SENIN_APP_ID";
```

Build al:

```bash
cargo build --release
```

### Çalıştırma

```bash
./target/release/stremio-discord-rpc
```

### macOS Servis Olarak Kurulum (Startup'ta Otomatik Başlat)

LaunchAgent plist dosyası oluştur:

```bash
cat > ~/Library/LaunchAgents/com.stremio-discord-rpc.plist << 'EOF'
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>com.stremio-discord-rpc</string>
    <key>ProgramArguments</key>
    <array>
        <string>/FULL/PATH/TO/stremio-discord-rpc</string>
    </array>
    <key>RunAtLoad</key>
    <true/>
    <key>KeepAlive</key>
    <true/>
    <key>StandardOutPath</key>
    <string>/tmp/stremio-discord-rpc.log</string>
    <key>StandardErrorPath</key>
    <string>/tmp/stremio-discord-rpc.log</string>
</dict>
</plist>
EOF
```

Servisi başlat:

```bash
launchctl load ~/Library/LaunchAgents/com.stremio-discord-rpc.plist
```

Servisi durdur:

```bash
launchctl unload ~/Library/LaunchAgents/com.stremio-discord-rpc.plist
```

## Image Server (Opsiyonel)

Poster görsellerinin köşelerini yuvarlatmak için basit bir Node.js image server kullanılır. `image-server/` dizininde bulunur.

### Docker ile çalıştırma

```bash
cd image-server
docker build -t stremio-rpc-img .
docker run -p 3000:3000 stremio-rpc-img
```

Kullanım:

```
GET /rounded?url=<image_url>&r=<radius>&s=<size>
```

- `url` — Kaynak görsel URL'si
- `r` — Köşe yarıçapı (varsayılan: 30)
- `s` — Görsel boyutu (varsayılan: 300)

`src/main.rs` dosyasındaki `round_image_url` fonksiyonunda image server URL'sini kendi sunucunla değiştir.

## Lisans

MIT
