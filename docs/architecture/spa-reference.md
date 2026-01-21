# Reference Architecture: Spa Controller

This reference implementation demonstrates a concise use of SovelmaOS: **Zigbee Endpoint Aggregation**.

## Hardware
- **Device**: M5Stack NanoC6 (ESP32-C6).
- **Radios**: WiFi 6 (Internet/OTA), 802.15.4 (Zigbee).

## Architecture

```mermaid
graph LR
    IKEA[IKEA Dirigera Hub] -- Zigbee --> SovelmaOS
    SovelmaOS -- TCP/WiFi --> Balboa[Balboa WiFi Module]

    subgraph "SovelmaOS (ESP32-C6)"
        direction TB
        Kernel[Kernel (Rust)]
        
        subgraph "WASM Module: Spa Bridge"
            Logic[Bridge Logic]
            VirtualDev[Virtual Devices]
        end
        
        Logic -- Host Fn --> Kernel
        Kernel -- Zigbee Stack --> IKEA
        Kernel -- TCP Stack --> Balboa
    end
```

## Implementation Pattern

1.  **Zigbee Stack (Kernel)**: The robust `esp-zigbee-sdk` runs as a kernel service or privileged driver, handling the timing-critical 802.15.4 PHY/MAC layers.
2.  **Protocol Bridge (WASM)**:
    -   **Input**: Zigbee commands (e.g., `Light On`) via Host Function / IPC.
    -   **Logic**: Translates `Light On` -> Balboa Protocol (`7E 05 0A BF 11 ...`).
    -   **Output**: Sends TCP packet to Balboa module via `sp_net_send`.
    -   **State**: Maintains local state mapping (e.g., Pump Cooldown logic).

## Why This Architecture?
- **Stability**: If the Bridge WASM module crashes (e.g., parsing error), the Zigbee radio stays connected, and the OS restarts the module in milliseconds.
- **Safety**: "Pump Cooldown" logic is verified in userspace but enforced by the module supervisor if necessary.
- **Updates**: Provide OTA updates to the Bridge logic without reflashing the customized Zigbee kernel.
