# SovelmaOS & Spa Controller - Quick Start Guide
## Getting Started for Both Projects

---

## Project Overview

This package contains specifications for two related projects:

| Project | Description | Target Hardware |
|---------|-------------|-----------------|
| **SovelmaOS OS** | Microkernel OS with WASM userspace | ESP32-C6, x86_64 |
| **Spa Controller** | Balboa hot tub → IKEA Zigbee bridge | ESP32-C6 (M5Stack NanoC6) |

**Recommendation:** Start with the Spa Controller - it's self-contained and immediately useful. SovelmaOS is a larger undertaking.

---

## Part 1: Spa Controller Quick Start

### What You Need

```
Hardware:
  □ M5Stack NanoC6 (or ESP32-C6-DevKitC)     ~$10
  □ USB-C cable
  □ IKEA Dirigera hub (already set up)
  
Software:
  □ ESP-IDF v5.2+
  □ esp-zigbee-sdk
  
Existing:
  □ Balboa spa with WiFi module installed
  □ WiFi network (2.4GHz)
```

### 10-Minute Setup

```bash
# 1. Install ESP-IDF (if not already)
mkdir -p ~/esp
cd ~/esp
git clone -b v5.2 --recursive https://github.com/espressif/esp-idf.git
cd esp-idf && ./install.sh esp32c6
source export.sh

# 2. Get Zigbee SDK
cd ~/esp
git clone https://github.com/espressif/esp-zigbee-sdk.git
export ESP_ZIGBEE_SDK_PATH=~/esp/esp-zigbee-sdk

# 3. Create project
mkdir -p ~/projects/spa-controller
cd ~/projects/spa-controller

# 4. Create minimal project structure
# (See templates below)
```

### Minimal main.c Template

```c
#include <stdio.h>
#include "freertos/FreeRTOS.h"
#include "freertos/task.h"
#include "esp_log.h"
#include "nvs_flash.h"
#include "esp_wifi.h"
#include "esp_zb_core.h"

static const char *TAG = "SPA";

// Your WiFi credentials
#define WIFI_SSID "YourNetwork"
#define WIFI_PASS "YourPassword"

// Spa status (simplified)
typedef struct {
    uint8_t temp;
    bool lights;
    bool pump;
} spa_status_t;

static spa_status_t spa_status = {0};

void app_main(void) {
    ESP_LOGI(TAG, "Spa Controller Starting...");
    
    // Initialize NVS
    ESP_ERROR_CHECK(nvs_flash_init());
    
    // TODO: Initialize WiFi
    // TODO: Connect to Balboa
    // TODO: Initialize Zigbee
    
    while (1) {
        ESP_LOGI(TAG, "Temp: %d°F, Lights: %s, Pump: %s",
                 spa_status.temp,
                 spa_status.lights ? "ON" : "OFF",
                 spa_status.pump ? "ON" : "OFF");
        vTaskDelay(pdMS_TO_TICKS(5000));
    }
}
```

### Build & Flash

```bash
# Configure
idf.py set-target esp32c6

# Build
idf.py build

# Flash and monitor
idf.py -p /dev/ttyUSB0 flash monitor
```

### Test Balboa Connection First

Before adding Zigbee, test that you can connect to your spa:

```c
// test_balboa.c - Minimal Balboa connection test

#include "lwip/sockets.h"

#define BALBOA_PORT 4257

int connect_to_spa(const char *ip) {
    int sock = socket(AF_INET, SOCK_STREAM, 0);
    
    struct sockaddr_in addr = {
        .sin_family = AF_INET,
        .sin_port = htons(BALBOA_PORT),
    };
    inet_pton(AF_INET, ip, &addr.sin_addr);
    
    if (connect(sock, (struct sockaddr *)&addr, sizeof(addr)) < 0) {
        ESP_LOGE(TAG, "Connect failed");
        return -1;
    }
    
    ESP_LOGI(TAG, "Connected to spa!");
    return sock;
}

void read_spa_status(int sock) {
    uint8_t buf[64];
    int len = recv(sock, buf, sizeof(buf), 0);
    
    if (len > 0) {
        // Find status message (7E ... FF AF 13 ... 7E)
        for (int i = 0; i < len - 5; i++) {
            if (buf[i] == 0x7E && buf[i+2] == 0xFF && 
                buf[i+3] == 0xAF && buf[i+4] == 0x13) {
                
                uint8_t temp = buf[i+5];
                ESP_LOGI(TAG, "Water temp: %d°F", temp);
                return;
            }
        }
    }
}
```

---

## Part 2: SovelmaOS OS Quick Start

### Phase 1: Hello World Kernel (Week 1)

Start with the absolute minimum: boot and print to UART.

```bash
# Create project structure
mkdir -p ~/projects/SovelmaOS/{kernel,hal,modules}
cd ~/projects/SovelmaOS

# Initialize Cargo workspace
cat > Cargo.toml << 'EOF'
[workspace]
members = ["kernel", "hal"]
resolver = "2"

[profile.release]
opt-level = "z"
lto = true
panic = "abort"
EOF

# Create kernel crate
cd kernel
cargo init --lib
```

### Minimal Kernel (ESP32-C6)

```rust
// kernel/src/lib.rs
#![no_std]
#![no_main]

use esp_hal::prelude::*;

#[entry]
fn main() -> ! {
    let peripherals = esp_hal::init(esp_hal::Config::default());
    
    let mut uart0 = esp_hal::uart::Uart::new(
        peripherals.UART0,
        esp_hal::uart::Config::default(),
    );
    
    loop {
        uart0.write_str("SovelmaOS booting...\n").ok();
        esp_hal::delay::Delay::new().delay_millis(1000);
    }
}

#[panic_handler]
fn panic(_: &core::panic::PanicInfo) -> ! {
    loop {}
}
```

### Kernel Cargo.toml

```toml
# kernel/Cargo.toml
[package]
name = "SovelmaOS-kernel"
version = "0.1.0"
edition = "2021"

[dependencies]
esp-hal = { version = "0.22", features = ["esp32c6"] }
esp-backtrace = { version = "0.14", features = ["esp32c6", "panic-handler", "println"] }

[features]
default = []
```

### Build Configuration

```toml
# .cargo/config.toml
[build]
target = "riscv32imac-unknown-none-elf"

[target.riscv32imac-unknown-none-elf]
runner = "espflash flash --monitor"
rustflags = [
    "-C", "link-arg=-Tlinkall.x",
]

[env]
ESP_LOG = "info"
```

### Development Phases

| Phase | Goal | Duration |
|-------|------|----------|
| 1 | Boot + UART | 1 week |
| 2 | Basic scheduler | 2 weeks |
| 3 | Add wasm3 runtime | 1 week |
| 4 | First WASM module runs | 1 week |
| 5 | Add smoltcp networking | 2 weeks |
| 6 | OTA module loading | 2 weeks |

---

## Part 3: Reference Cards

### Balboa Protocol Quick Reference

```
Message Format:
  7E [LEN] [PAYLOAD...] [CRC] 7E

Status Message (FF AF 13):
  Byte 3:  Current temp (°F), 0xFF = unknown
  Byte 11: Pumps bitmask (2 bits each: 0=off, 1=low, 2=high)
  Byte 14: Light 1 (bits 0-1)
  Byte 16: Set temp (°F)
  Byte 17: bit 4 = heating active

Toggle Command:
  [0x0A] [0xBF] [0x11] [ITEM]
  
  Items: 0x04=Pump1, 0x05=Pump2, 0x11=Light1, 0x0C=Blower

Set Temp Command:
  [0x0A] [0xBF] [0x20] [TEMP]
  
  TEMP: 80-104 (°F)

CRC-8:
  Poly=0x07, Init=0x02, XorOut=0x02
```

### Zigbee Endpoints Quick Reference

```
Endpoint 1 - Temperature Sensor:
  Profile: 0x0104 (Home Automation)
  Device:  0x0302 (Temperature Sensor)
  Cluster: 0x0402 (Temperature Measurement)
  
  Attribute 0x0000: MeasuredValue (int16, 0.01°C)

Endpoint 2 - Light:
  Profile: 0x0104
  Device:  0x0100 (On/Off Light)
  Cluster: 0x0006 (On/Off)
  
  Attribute 0x0000: OnOff (bool)

Endpoint 3 - Switch:
  Profile: 0x0104
  Device:  0x0103 (On/Off Switch)
  Cluster: 0x0006 (On/Off)
```

### SovelmaOS Host Functions Quick Reference

```
Core:
  sp_yield()              - Yield to scheduler
  sp_sleep(ms)            - Sleep for N milliseconds
  sp_time() -> u64        - Get system time
  sp_log(level, ptr, len) - Log message

Filesystem:
  sp_fs_open(path, len, flags) -> fd
  sp_fs_read(fd, buf, len) -> bytes_read
  sp_fs_write(fd, ptr, len) -> bytes_written
  sp_fs_close(fd)

Network:
  sp_net_socket(proto) -> sock
  sp_net_connect(sock, addr, port)
  sp_net_send(sock, ptr, len) -> bytes_sent
  sp_net_recv(sock, buf, len) -> bytes_recv
  sp_net_close(sock)

UART:
  sp_uart_open(port, baud) -> handle
  sp_uart_write(handle, ptr, len)
  sp_uart_read(handle, buf, len)
```

---

## Part 4: File Manifest

```
specs/
├── 01-architecture.md             # Core OS specification
│   • Architecture overview
│   • Scheduler design
│   • Capability system
│   • WASM runtime
│   • Host functions API
│   • Boot sequence
│
├── 02-filesystem.md               # Filesystem specification  
│   • VFS layer design
│   • Capability-based access
│   • littlefs integration
│   • ramfs implementation
│
├── 03-balboa-controller.md        # Reference application spec
│   • System architecture
│   • Balboa protocol details
│   • Zigbee endpoint design
│
└── ../getting-started.md          # This file
    • Getting started guides
    • Reference cards
    • Development phases
```

---

## Part 5: Recommended Order of Development

### For Spa Controller (Start Here!)

```
Week 1:
  □ Set up ESP-IDF environment
  □ Flash basic "hello world" to ESP32-C6
  □ Connect to WiFi
  □ Find spa IP (DHCP scan for 00:15:27 MAC)

Week 2:
  □ TCP connect to spa port 4257
  □ Parse status messages
  □ Log temperature to serial

Week 3:
  □ Add Zigbee stack
  □ Create temperature sensor endpoint
  □ Join IKEA network
  □ See temperature in IKEA app

Week 4:
  □ Add light control endpoint
  □ Add pump control endpoint
  □ Test bidirectional control
  □ Add safety cooldowns

Week 5:
  □ Polish, error handling
  □ Watchdog
  □ 3D print enclosure
  □ Deploy!
```

### For SovelmaOS OS (After Spa Controller)

```
Month 1:
  □ Boot to UART output
  □ Basic cooperative scheduler
  □ Memory region allocator

Month 2:
  □ Integrate wasm3
  □ Implement 5 host functions
  □ Run trivial WASM module

Month 3:
  □ Add smoltcp
  □ DHCP at boot
  □ TCP sockets from WASM

Month 4:
  □ littlefs filesystem
  □ OTA module download
  □ Supervisor restart

Month 5:
  □ Port spa controller to SovelmaOS
  □ Run as WASM module
  □ Compare with native
```

---

## Resources

### ESP32 / ESP-IDF
- https://docs.espressif.com/projects/esp-idf/
- https://github.com/esp-rs (Rust on ESP)

### Zigbee
- https://github.com/espressif/esp-zigbee-sdk
- https://zigbeealliance.org/solution/zigbee/

### Balboa Protocol
- https://github.com/ccutrer/balboa_worldwide_app
- https://github.com/cribskip/esp8266_spa

### SovelmaOS Dependencies
- https://github.com/wasm3/wasm3
- https://github.com/smoltcp-rs/smoltcp
- https://github.com/littlefs-project/littlefs

---

*Happy hacking!*
