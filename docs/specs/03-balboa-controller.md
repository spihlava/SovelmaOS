# Balboa Spa Controller Specification
## ESP32-C6 / Zigbee / IKEA Dirigera Integration
## Version 0.1.0 - Draft

---

## 1. Overview

### 1.1 Purpose
This document specifies a smart home controller for Balboa-based hot tubs/spas, using an ESP32-C6 to bridge between the Balboa WiFi protocol and Zigbee (IKEA Dirigera hub).

### 1.2 Features
- Read spa status (temperature, heating, pump state)
- Control spa (lights, pumps, temperature setpoint)
- Native integration with IKEA Home Smart app
- No cloud dependency
- Endpoint aggregation (appears as multiple devices)

### 1.3 Hardware Target
- **MCU**: ESP32-C6 (M5Stack NanoC6 or similar)
- **Connectivity**: WiFi + Zigbee (802.15.4)
- **No RS-485 required** - connects to existing Balboa WiFi module

---

## 2. System Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                      IKEA Home Smart App                        │
│                                                                 │
│    ┌────────────┐  ┌────────────┐  ┌────────────┐             │
│    │   Temp     │  │   Light    │  │   Pump     │             │
│    │  Sensor    │  │  Switch    │  │  Switch    │             │
│    └─────┬──────┘  └─────┬──────┘  └─────┬──────┘             │
└──────────┼───────────────┼───────────────┼──────────────────────┘
           │               │               │
           └───────────────┴───────────────┘
                           │
                      Zigbee 3.0
                           │
┌──────────────────────────┴──────────────────────────────────────┐
│                    IKEA Dirigera Hub                            │
│                    (Zigbee Coordinator)                         │
└──────────────────────────┬──────────────────────────────────────┘
                           │
                      Zigbee 3.0
                           │
┌──────────────────────────┴──────────────────────────────────────┐
│                     ESP32-C6 Controller                         │
│                                                                 │
│  ┌─────────────────────────────────────────────────────────┐   │
│  │                  Zigbee Stack                            │   │
│  │                                                          │   │
│  │  Endpoint 1: Temperature Sensor                         │   │
│  │    • Cluster: Temperature Measurement (0x0402)          │   │
│  │    • Reports: Current water temperature                 │   │
│  │                                                          │   │
│  │  Endpoint 2: On/Off Light                               │   │
│  │    • Cluster: On/Off (0x0006)                           │   │
│  │    • Controls: Spa lights                               │   │
│  │                                                          │   │
│  │  Endpoint 3: On/Off Switch                              │   │
│  │    • Cluster: On/Off (0x0006)                           │   │
│  │    • Controls: Pump 1 (Jets)                            │   │
│  │                                                          │   │
│  │  Endpoint 4: Thermostat (Optional)                      │   │
│  │    • Cluster: Thermostat (0x0201)                       │   │
│  │    • Controls: Set temperature                          │   │
│  └─────────────────────────────────────────────────────────┘   │
│                           │                                     │
│                   Protocol Bridge                               │
│                           │                                     │
│  ┌─────────────────────────────────────────────────────────┐   │
│  │                  Balboa Client                           │   │
│  │                                                          │   │
│  │  • TCP connection to Balboa WiFi module                 │   │
│  │  • Parse status messages (FF AF 13)                     │   │
│  │  • Send control commands (toggle, set temp)             │   │
│  │  • CRC-8 calculation                                    │   │
│  └─────────────────────────────────────────────────────────┘   │
│                           │                                     │
│                        WiFi                                     │
└───────────────────────────┴─────────────────────────────────────┘
                            │
                       TCP/IP
                            │
┌───────────────────────────┴─────────────────────────────────────┐
│                  Balboa WiFi Module                             │
│                  (Inside Spa Pack)                              │
│                                                                 │
│  • IP: Obtained via DHCP                                       │
│  • Port: 4257                                                  │
│  • MAC prefix: 00:15:27                                        │
│  • Broadcasts status ~1/second                                 │
└─────────────────────────────────────────────────────────────────┘
```

---

## 3. Hardware Specification

### 3.1 Required Components

| Component | Specification | Notes |
|-----------|---------------|-------|
| ESP32-C6 | M5Stack NanoC6 or DevKitC | Must have WiFi + 802.15.4 |
| Enclosure | IP65 or better | Near spa, not in water |
| Power | USB-C or 5V adapter | External power recommended |

### 3.2 M5Stack NanoC6 Pinout

```
┌─────────────────────────────────────────┐
│           M5Stack NanoC6                │
│                                         │
│  USB-C ────┐                           │
│            │                           │
│  ┌─────────┴─────────┐                 │
│  │                   │                 │
│  │    ESP32-C6       │                 │
│  │                   │                 │
│  │  WiFi: Internal   │                 │
│  │  802.15.4: Internal │               │
│  │                   │                 │
│  │  GPIO1: User LED  │                 │
│  │  GPIO2: User BTN  │                 │
│  │                   │                 │
│  └───────────────────┘                 │
│                                         │
│  No external wiring required!          │
│  (Using existing Balboa WiFi module)   │
└─────────────────────────────────────────┘
```

### 3.3 Optional Status Indicators

| GPIO | Function | Description |
|------|----------|-------------|
| GPIO1 | Status LED | Blink patterns for state |
| GPIO2 | Button | Manual reset / pairing |

**LED Patterns:**
- Slow blink (1s): Searching for spa
- Fast blink (200ms): Connecting to Zigbee network
- Solid: Connected and operational
- Double blink: Error condition

---

## 4. Balboa Protocol Specification

### 4.1 Discovery

```
UDP Broadcast to 255.255.255.255:30303

Response (unicast):
  Line 1: "BWGSPA\r\n"
  Line 2: "00:15:27:xx:xx:xx\r\n"  (MAC address)

Alternative: Scan DHCP leases for MAC prefix 00:15:27
```

### 4.2 Connection

```
TCP connection to <spa_ip>:4257
No authentication required
Spa immediately starts sending status updates
```

### 4.3 Message Format

```
┌────┬────────┬────────────────────────┬─────┬────┐
│ 7E │ Length │        Payload         │ CRC │ 7E │
└────┴────────┴────────────────────────┴─────┴────┘

7E: Start/end of frame (0x7E)
Length: Number of bytes from Length to CRC (inclusive)
Payload: Variable, depends on message type
CRC: CRC-8 of bytes from Length to end of Payload

CRC-8 Parameters:
  Polynomial: 0x07
  Initial: 0x02
  XOR Out: 0x02
  Reflect In: false
  Reflect Out: false
```

### 4.4 Status Update Message (FF AF 13)

Received approximately once per second.

```
Byte  Offset  Description
────  ──────  ─────────────────────────────────────
0     0x00    Channel: 0xFF (broadcast)
1     0x01    Message type high: 0xAF
2     0x02    Message type low: 0x13
3     0x03    Current temperature (°F), 0xFF = unknown
4     0x04    Hour (bit 7 = 24-hour mode)
5     0x05    Minute
6     0x06    Heating mode:
              0x00 = Ready
              0x01 = Rest
              0x02 = Ready-in-Rest
7     0x07    Reminder byte 1
8     0x08    Temperature range:
              0x00 = Low (80-99°F)
              0x01 = High (80-104°F)
9     0x09    Sensor A temperature
10    0x0A    Sensor B temperature
11    0x0B    Pump status (bitmask):
              Bits 0-1: Pump 1 (0=off, 1=low, 2=high)
              Bits 2-3: Pump 2
              Bits 4-5: Pump 3
              Bits 6-7: Pump 4
12    0x0C    Circulation pump and misc:
              Bit 1: Circ pump running
13    0x0D    Blower status
14    0x0E    Light 1 status:
              Bits 0-1: State (0=off, 1-3=on/levels)
15    0x0F    Light 2 status
16    0x10    Set temperature (°F)
17    0x11    Flags:
              Bit 4: Heater is currently active
18-   0x12+   Additional bytes (model dependent)
```

### 4.5 Command Messages

#### 4.5.1 Toggle Item (0xBF 0x11)

Toggles a spa feature on/off.

```
Payload: [channel] [0xBF] [0x11] [item_code]

Item Codes:
  0x04 = Pump 1
  0x05 = Pump 2
  0x06 = Pump 3
  0x07 = Pump 4
  0x08 = Pump 5
  0x09 = Pump 6
  0x0C = Blower
  0x11 = Light 1
  0x12 = Light 2
  0x16 = Aux 1
  0x17 = Aux 2
  0x50 = Hold (temperature hold)
  0x51 = Temperature Range toggle

Example: Toggle Pump 1
  Channel = 0x0A (WiFi module channel)
  Full message: 7E 05 0A BF 11 04 [CRC] 7E
```

#### 4.5.2 Set Temperature (0xBF 0x20)

```
Payload: [channel] [0xBF] [0x20] [temperature]

Temperature: Desired temp in °F (80-104)

Example: Set to 102°F
  7E 05 0A BF 20 66 [CRC] 7E
```

#### 4.5.3 Set Time (0xBF 0x21)

```
Payload: [channel] [0xBF] [0x21] [hour] [minute]

Hour: 0-23 (bit 7 = 1 for 24-hour mode)
Minute: 0-59
```

#### 4.5.4 Set Heating Mode (0xBF 0x22)

```
Payload: [channel] [0xBF] [0x22] [mode]

Mode:
  0x00 = Ready
  0x01 = Rest
```

### 4.6 CRC-8 Implementation

```c
uint8_t crc8_balboa(const uint8_t *data, size_t len) {
    uint8_t crc = 0x02;  // Initial value
    
    for (size_t i = 0; i < len; i++) {
        crc ^= data[i];
        for (int j = 0; j < 8; j++) {
            if (crc & 0x80) {
                crc = (crc << 1) ^ 0x07;
            } else {
                crc <<= 1;
            }
        }
    }
    
    return crc ^ 0x02;  // XOR out
}

// Calculate CRC for message (excluding 0x7E markers)
// CRC covers: length byte + payload bytes
uint8_t calculate_message_crc(const uint8_t *msg, size_t len) {
    // msg starts with length byte, includes payload, excludes CRC itself
    return crc8_balboa(msg, len);
}
```

---

## 5. Zigbee Specification

### 5.1 Device Configuration

```c
// Zigbee device type
#define DEVICE_TYPE_COMBO         0x0000

// Endpoints
#define ENDPOINT_TEMP_SENSOR      1
#define ENDPOINT_LIGHT            2
#define ENDPOINT_PUMP_SWITCH      3
#define ENDPOINT_THERMOSTAT       4  // Optional

// Profile
#define PROFILE_HA                0x0104  // Home Automation
```

### 5.2 Endpoint 1: Temperature Sensor

```c
// Cluster: Temperature Measurement (0x0402)
typedef struct {
    int16_t measured_value;    // Temperature in 0.01°C
    int16_t min_measured;      // Minimum
    int16_t max_measured;      // Maximum
    uint16_t tolerance;        // Tolerance
} temp_measurement_attr_t;

// Conversion: °F to Zigbee format
int16_t fahrenheit_to_zigbee(uint8_t temp_f) {
    // Convert to Celsius, then to 0.01°C units
    float celsius = (temp_f - 32.0f) * 5.0f / 9.0f;
    return (int16_t)(celsius * 100);
}

// Report interval: 60 seconds or on change > 0.5°C
#define TEMP_REPORT_INTERVAL_S    60
#define TEMP_REPORT_CHANGE        50  // 0.5°C in 0.01°C units
```

### 5.3 Endpoint 2: Light Control

```c
// Cluster: On/Off (0x0006)
typedef struct {
    bool on_off;
} on_off_attr_t;

// Command handling
void on_light_on_command(void) {
    balboa_send_toggle(ITEM_LIGHT1);
    // Wait for status update to confirm
}

void on_light_off_command(void) {
    // Only send if currently on
    if (spa_status.lights[0]) {
        balboa_send_toggle(ITEM_LIGHT1);
    }
}
```

### 5.4 Endpoint 3: Pump Switch

```c
// Cluster: On/Off (0x0006)
// Same structure as light

// Safety: Pump cooldown
#define PUMP_COOLDOWN_MS    10000

static uint32_t last_pump_toggle = 0;

void on_pump_command(bool on) {
    uint32_t now = esp_timer_get_time() / 1000;
    
    if (now - last_pump_toggle < PUMP_COOLDOWN_MS) {
        ESP_LOGW(TAG, "Pump cooldown active, ignoring");
        return;
    }
    
    // Only toggle if state differs
    bool current = spa_status.pumps[0] != PUMP_OFF;
    if (on != current) {
        balboa_send_toggle(ITEM_PUMP1);
        last_pump_toggle = now;
    }
}
```

### 5.5 Endpoint 4: Thermostat (Optional)

```c
// Cluster: Thermostat (0x0201)
typedef struct {
    int16_t local_temp;           // Current temp (0.01°C)
    int16_t occupied_heating_setpoint;  // Set temp (0.01°C)
    int16_t min_heat_setpoint;    // 26.67°C (80°F)
    int16_t max_heat_setpoint;    // 40.00°C (104°F)
    uint8_t system_mode;          // 0=Off, 4=Heat
} thermostat_attr_t;

// Set temperature command
void on_setpoint_command(int16_t setpoint_zigbee) {
    // Convert from 0.01°C to °F
    float celsius = setpoint_zigbee / 100.0f;
    float fahrenheit = (celsius * 9.0f / 5.0f) + 32.0f;
    uint8_t temp_f = (uint8_t)(fahrenheit + 0.5f);
    
    // Clamp to valid range
    if (temp_f < 80) temp_f = 80;
    if (temp_f > 104) temp_f = 104;
    
    balboa_send_set_temp(temp_f);
}
```

### 5.6 Zigbee Network Join

```c
void zigbee_init(void) {
    esp_zb_platform_config_t config = {
        .radio_config = ESP_ZB_DEFAULT_RADIO_CONFIG(),
        .host_config = ESP_ZB_DEFAULT_HOST_CONFIG(),
    };
    ESP_ERROR_CHECK(esp_zb_platform_config(&config));
    
    // Create endpoint list
    esp_zb_ep_list_t *ep_list = esp_zb_ep_list_create();
    
    // Add temperature sensor endpoint
    esp_zb_cluster_list_t *temp_clusters = esp_zb_zcl_cluster_list_create();
    esp_zb_temperature_sensor_cluster_add(temp_clusters, &temp_sensor_cfg);
    esp_zb_ep_list_add_ep(ep_list, temp_clusters, ENDPOINT_TEMP_SENSOR, 
                          PROFILE_HA, ESP_ZB_HA_TEMPERATURE_SENSOR_DEVICE_ID);
    
    // Add light endpoint
    esp_zb_cluster_list_t *light_clusters = esp_zb_zcl_cluster_list_create();
    esp_zb_on_off_cluster_add(light_clusters, &on_off_cfg);
    esp_zb_ep_list_add_ep(ep_list, light_clusters, ENDPOINT_LIGHT,
                          PROFILE_HA, ESP_ZB_HA_ON_OFF_LIGHT_DEVICE_ID);
    
    // Add pump switch endpoint
    esp_zb_cluster_list_t *pump_clusters = esp_zb_zcl_cluster_list_create();
    esp_zb_on_off_cluster_add(pump_clusters, &on_off_cfg);
    esp_zb_ep_list_add_ep(ep_list, pump_clusters, ENDPOINT_PUMP_SWITCH,
                          PROFILE_HA, ESP_ZB_HA_ON_OFF_SWITCH_DEVICE_ID);
    
    // Register device
    esp_zb_device_register(ep_list);
    
    // Start Zigbee stack
    ESP_ERROR_CHECK(esp_zb_start(false));
}

// Handle Zigbee signals
void esp_zb_app_signal_handler(esp_zb_app_signal_t *signal) {
    switch (signal->sig_type) {
        case ESP_ZB_ZDO_SIGNAL_SKIP_STARTUP:
            esp_zb_bdb_start_top_level_commissioning(ESP_ZB_BDB_MODE_NETWORK_STEERING);
            break;
            
        case ESP_ZB_BDB_SIGNAL_STEERING:
            if (signal->status == ESP_OK) {
                ESP_LOGI(TAG, "Joined Zigbee network");
            } else {
                ESP_LOGW(TAG, "Network steering failed, retrying...");
                esp_zb_bdb_start_top_level_commissioning(ESP_ZB_BDB_MODE_NETWORK_STEERING);
            }
            break;
            
        default:
            break;
    }
}
```

---

## 6. Software Architecture

### 6.1 Component Diagram

```
┌─────────────────────────────────────────────────────────────────┐
│                        Main Task                                │
│                                                                 │
│  ┌─────────────────────────────────────────────────────────┐   │
│  │                    State Machine                         │   │
│  │                                                          │   │
│  │  INIT → WIFI_CONNECT → SPA_DISCOVER → SPA_CONNECT →     │   │
│  │  ZIGBEE_JOIN → RUNNING                                  │   │
│  │                                                          │   │
│  │  Any state can transition to ERROR on failure           │   │
│  └─────────────────────────────────────────────────────────┘   │
├─────────────────────────────────────────────────────────────────┤
│                                                                 │
│  ┌──────────────────┐  ┌──────────────────┐                   │
│  │   Balboa Task    │  │   Zigbee Task    │                   │
│  │                  │  │                  │                   │
│  │  • TCP socket    │  │  • esp_zigbee    │                   │
│  │  • Rx parser     │  │  • Attribute     │                   │
│  │  • Tx commands   │  │    reporting     │                   │
│  │  • Status cache  │  │  • Command       │                   │
│  │                  │  │    handling      │                   │
│  └────────┬─────────┘  └────────┬─────────┘                   │
│           │                     │                              │
│           └──────────┬──────────┘                              │
│                      │                                         │
│           ┌──────────▼──────────┐                             │
│           │    Shared State     │                             │
│           │                     │                             │
│           │  • spa_status       │                             │
│           │  • command_queue    │                             │
│           │  • mutex protected  │                             │
│           └─────────────────────┘                             │
└─────────────────────────────────────────────────────────────────┘
```

### 6.2 Data Structures

```c
// Pump state enumeration
typedef enum {
    PUMP_OFF = 0,
    PUMP_LOW = 1,
    PUMP_HIGH = 2,
} pump_state_t;

// Heating mode
typedef enum {
    HEAT_MODE_READY = 0,
    HEAT_MODE_REST = 1,
    HEAT_MODE_READY_IN_REST = 2,
} heat_mode_t;

// Spa status (updated from Balboa messages)
typedef struct {
    uint8_t current_temp;        // °F, 0 = unknown
    uint8_t set_temp;            // °F
    bool temp_unknown;           // True if temp reading unavailable
    
    heat_mode_t heat_mode;
    bool heating_active;
    bool temp_range_high;        // True = High range (80-104)
    
    pump_state_t pumps[4];
    bool circ_pump;
    bool blower;
    bool lights[2];
    
    uint8_t hour;
    uint8_t minute;
    
    uint32_t last_update_ms;     // Timestamp of last status
    bool connected;
} spa_status_t;

// Command to send to spa
typedef struct {
    enum {
        CMD_TOGGLE_ITEM,
        CMD_SET_TEMP,
        CMD_SET_TIME,
        CMD_SET_MODE,
    } type;
    
    union {
        uint8_t item_code;       // For toggle
        uint8_t temperature;     // For set temp
        struct { uint8_t hour; uint8_t minute; } time;
        uint8_t mode;            // For set mode
    };
} spa_command_t;

// Global state
extern spa_status_t spa_status;
extern QueueHandle_t command_queue;
extern SemaphoreHandle_t status_mutex;
```

### 6.3 Balboa Task

```c
#define BALBOA_PORT         4257
#define BALBOA_BUFFER_SIZE  64
#define STATUS_TIMEOUT_MS   5000

static void balboa_task(void *arg) {
    int sock = -1;
    uint8_t rx_buffer[BALBOA_BUFFER_SIZE];
    size_t rx_len = 0;
    
    while (1) {
        // Connect if needed
        if (sock < 0) {
            sock = connect_to_spa();
            if (sock < 0) {
                vTaskDelay(pdMS_TO_TICKS(5000));
                continue;
            }
        }
        
        // Check for commands to send
        spa_command_t cmd;
        if (xQueueReceive(command_queue, &cmd, 0) == pdTRUE) {
            send_command(sock, &cmd);
        }
        
        // Receive data
        int len = recv(sock, rx_buffer + rx_len, 
                       BALBOA_BUFFER_SIZE - rx_len, MSG_DONTWAIT);
        
        if (len > 0) {
            rx_len += len;
            
            // Try to parse complete messages
            size_t consumed = parse_messages(rx_buffer, rx_len);
            
            if (consumed > 0) {
                // Shift buffer
                memmove(rx_buffer, rx_buffer + consumed, rx_len - consumed);
                rx_len -= consumed;
            }
        } else if (len == 0) {
            // Connection closed
            ESP_LOGW(TAG, "Spa connection closed");
            close(sock);
            sock = -1;
        }
        
        // Check for timeout
        uint32_t now = xTaskGetTickCount() * portTICK_PERIOD_MS;
        if (now - spa_status.last_update_ms > STATUS_TIMEOUT_MS) {
            xSemaphoreTake(status_mutex, portMAX_DELAY);
            spa_status.connected = false;
            xSemaphoreGive(status_mutex);
        }
        
        vTaskDelay(pdMS_TO_TICKS(50));
    }
}

static size_t parse_messages(uint8_t *buf, size_t len) {
    size_t consumed = 0;
    
    while (consumed < len) {
        // Find start marker
        if (buf[consumed] != 0x7E) {
            consumed++;
            continue;
        }
        
        // Need at least: 7E LEN ... CRC 7E
        if (consumed + 4 > len) break;
        
        uint8_t msg_len = buf[consumed + 1];
        size_t total_len = msg_len + 3;  // 7E + len + payload + crc + 7E
        
        if (consumed + total_len > len) break;  // Incomplete
        
        // Verify end marker
        if (buf[consumed + total_len - 1] != 0x7E) {
            consumed++;
            continue;
        }
        
        // Verify CRC
        uint8_t expected_crc = crc8_balboa(&buf[consumed + 1], msg_len);
        uint8_t actual_crc = buf[consumed + total_len - 2];
        
        if (expected_crc != actual_crc) {
            ESP_LOGW(TAG, "CRC mismatch");
            consumed++;
            continue;
        }
        
        // Parse payload
        uint8_t *payload = &buf[consumed + 2];
        if (payload[0] == 0xFF && payload[1] == 0xAF && payload[2] == 0x13) {
            parse_status_message(payload, msg_len - 1);
        }
        
        consumed += total_len;
    }
    
    return consumed;
}

static void parse_status_message(const uint8_t *payload, size_t len) {
    if (len < 18) return;
    
    xSemaphoreTake(status_mutex, portMAX_DELAY);
    
    spa_status.temp_unknown = (payload[3] == 0xFF);
    spa_status.current_temp = spa_status.temp_unknown ? 0 : payload[3];
    
    spa_status.hour = payload[4] & 0x7F;
    spa_status.minute = payload[5];
    spa_status.heat_mode = payload[6];
    spa_status.temp_range_high = (payload[8] != 0);
    
    uint8_t pump_byte = payload[11];
    spa_status.pumps[0] = (pump_byte >> 0) & 0x03;
    spa_status.pumps[1] = (pump_byte >> 2) & 0x03;
    spa_status.pumps[2] = (pump_byte >> 4) & 0x03;
    spa_status.pumps[3] = (pump_byte >> 6) & 0x03;
    
    spa_status.circ_pump = (payload[12] & 0x02) != 0;
    spa_status.blower = payload[13] != 0;
    spa_status.lights[0] = (payload[14] & 0x03) != 0;
    spa_status.lights[1] = (payload[15] & 0x03) != 0;
    spa_status.set_temp = payload[16];
    spa_status.heating_active = (payload[17] & 0x10) != 0;
    
    spa_status.last_update_ms = xTaskGetTickCount() * portTICK_PERIOD_MS;
    spa_status.connected = true;
    
    xSemaphoreGive(status_mutex);
    
    // Notify Zigbee task to update attributes
    xEventGroupSetBits(event_group, STATUS_UPDATED_BIT);
}

static void send_command(int sock, const spa_command_t *cmd) {
    uint8_t payload[8];
    size_t payload_len = 0;
    
    payload[payload_len++] = 0x0A;  // WiFi module channel
    payload[payload_len++] = 0xBF;
    
    switch (cmd->type) {
        case CMD_TOGGLE_ITEM:
            payload[payload_len++] = 0x11;
            payload[payload_len++] = cmd->item_code;
            break;
            
        case CMD_SET_TEMP:
            payload[payload_len++] = 0x20;
            payload[payload_len++] = cmd->temperature;
            break;
            
        case CMD_SET_TIME:
            payload[payload_len++] = 0x21;
            payload[payload_len++] = cmd->time.hour | 0x80;  // 24h mode
            payload[payload_len++] = cmd->time.minute;
            break;
            
        case CMD_SET_MODE:
            payload[payload_len++] = 0x22;
            payload[payload_len++] = cmd->mode;
            break;
    }
    
    // Build message
    uint8_t msg[16];
    size_t msg_len = 0;
    
    msg[msg_len++] = 0x7E;
    msg[msg_len++] = payload_len + 1;  // +1 for CRC
    memcpy(&msg[msg_len], payload, payload_len);
    msg_len += payload_len;
    msg[msg_len++] = crc8_balboa(&msg[1], payload_len + 1);
    msg[msg_len++] = 0x7E;
    
    send(sock, msg, msg_len, 0);
    
    ESP_LOGI(TAG, "Sent command type %d", cmd->type);
}
```

### 6.4 Zigbee Task

```c
static void zigbee_task(void *arg) {
    // Initialize Zigbee
    zigbee_init();
    
    // Wait for network join
    xEventGroupWaitBits(event_group, ZIGBEE_JOINED_BIT, 
                        pdFALSE, pdTRUE, portMAX_DELAY);
    
    int16_t last_reported_temp = 0;
    uint32_t last_report_time = 0;
    
    while (1) {
        // Wait for status update or timeout
        EventBits_t bits = xEventGroupWaitBits(
            event_group, STATUS_UPDATED_BIT,
            pdTRUE, pdFALSE, pdMS_TO_TICKS(1000));
        
        if (bits & STATUS_UPDATED_BIT) {
            xSemaphoreTake(status_mutex, portMAX_DELAY);
            
            // Update temperature attribute
            if (!spa_status.temp_unknown) {
                int16_t zigbee_temp = fahrenheit_to_zigbee(spa_status.current_temp);
                
                // Report if changed significantly or time elapsed
                uint32_t now = xTaskGetTickCount() * portTICK_PERIOD_MS;
                bool should_report = 
                    abs(zigbee_temp - last_reported_temp) >= TEMP_REPORT_CHANGE ||
                    now - last_report_time >= TEMP_REPORT_INTERVAL_S * 1000;
                
                if (should_report) {
                    esp_zb_zcl_set_attribute_val(
                        ENDPOINT_TEMP_SENSOR,
                        ESP_ZB_ZCL_CLUSTER_ID_TEMP_MEASUREMENT,
                        ESP_ZB_ZCL_CLUSTER_SERVER_ROLE,
                        ESP_ZB_ZCL_ATTR_TEMP_MEASUREMENT_VALUE_ID,
                        &zigbee_temp, false);
                    
                    last_reported_temp = zigbee_temp;
                    last_report_time = now;
                }
            }
            
            // Update light status
            bool light_on = spa_status.lights[0];
            esp_zb_zcl_set_attribute_val(
                ENDPOINT_LIGHT,
                ESP_ZB_ZCL_CLUSTER_ID_ON_OFF,
                ESP_ZB_ZCL_CLUSTER_SERVER_ROLE,
                ESP_ZB_ZCL_ATTR_ON_OFF_ON_OFF_ID,
                &light_on, false);
            
            // Update pump status
            bool pump_on = spa_status.pumps[0] != PUMP_OFF;
            esp_zb_zcl_set_attribute_val(
                ENDPOINT_PUMP_SWITCH,
                ESP_ZB_ZCL_CLUSTER_ID_ON_OFF,
                ESP_ZB_ZCL_CLUSTER_SERVER_ROLE,
                ESP_ZB_ZCL_ATTR_ON_OFF_ON_OFF_ID,
                &pump_on, false);
            
            xSemaphoreGive(status_mutex);
        }
    }
}

// Zigbee command callback
static esp_err_t zb_action_handler(esp_zb_core_action_callback_id_t callback_id,
                                    const void *message) {
    switch (callback_id) {
        case ESP_ZB_CORE_SET_ATTR_VALUE_CB_ID: {
            esp_zb_zcl_set_attr_value_message_t *msg = 
                (esp_zb_zcl_set_attr_value_message_t *)message;
            
            if (msg->info.cluster == ESP_ZB_ZCL_CLUSTER_ID_ON_OFF &&
                msg->attribute.id == ESP_ZB_ZCL_ATTR_ON_OFF_ON_OFF_ID) {
                
                bool on = *(bool *)msg->attribute.data.value;
                spa_command_t cmd = { .type = CMD_TOGGLE_ITEM };
                
                if (msg->info.dst_endpoint == ENDPOINT_LIGHT) {
                    // Only toggle if state differs
                    if (on != spa_status.lights[0]) {
                        cmd.item_code = 0x11;  // Light 1
                        xQueueSend(command_queue, &cmd, 0);
                    }
                } else if (msg->info.dst_endpoint == ENDPOINT_PUMP_SWITCH) {
                    bool current_on = spa_status.pumps[0] != PUMP_OFF;
                    if (on != current_on) {
                        cmd.item_code = 0x04;  // Pump 1
                        xQueueSend(command_queue, &cmd, 0);
                    }
                }
            }
            break;
        }
        
        default:
            break;
    }
    
    return ESP_OK;
}
```

---

## 7. Configuration

### 7.1 WiFi Credentials

Store in NVS (non-volatile storage):

```c
#define NVS_NAMESPACE "spa_ctrl"

typedef struct {
    char wifi_ssid[32];
    char wifi_pass[64];
    char spa_ip[16];        // Optional: fixed IP
    uint8_t spa_mac[6];     // Optional: for discovery filtering
} config_t;

void config_load(config_t *cfg);
void config_save(const config_t *cfg);
```

### 7.2 Build Configuration (sdkconfig)

```
# ESP-IDF configuration

# WiFi
CONFIG_ESP_WIFI_SSID="YourNetworkSSID"
CONFIG_ESP_WIFI_PASSWORD="YourPassword"

# Zigbee
CONFIG_ZB_ENABLED=y
CONFIG_ZB_RADIO_MODE_NATIVE=y
CONFIG_ZB_COORDINATOR=n

# Stack sizes
CONFIG_ESP_MAIN_TASK_STACK_SIZE=4096
CONFIG_BALBOA_TASK_STACK_SIZE=4096
CONFIG_ZIGBEE_TASK_STACK_SIZE=4096

# Logging
CONFIG_LOG_DEFAULT_LEVEL_INFO=y
```

---

## 8. Safety Considerations

### 8.1 Pump Protection

```c
// Prevent rapid pump cycling (motor damage)
#define PUMP_MIN_CYCLE_TIME_MS    10000

static uint32_t pump_last_toggle[4] = {0};

bool can_toggle_pump(int pump_num) {
    uint32_t now = esp_timer_get_time() / 1000;
    if (now - pump_last_toggle[pump_num] < PUMP_MIN_CYCLE_TIME_MS) {
        return false;
    }
    pump_last_toggle[pump_num] = now;
    return true;
}
```

### 8.2 Temperature Limits

```c
// Never allow setting temp above 104°F
#define MAX_SET_TEMP_F    104
#define MIN_SET_TEMP_F    80

uint8_t clamp_temperature(uint8_t requested) {
    if (requested > MAX_SET_TEMP_F) return MAX_SET_TEMP_F;
    if (requested < MIN_SET_TEMP_F) return MIN_SET_TEMP_F;
    return requested;
}
```

### 8.3 Watchdog

```c
// Reboot if spa connection lost for extended period
#define SPA_WATCHDOG_TIMEOUT_MS    (5 * 60 * 1000)  // 5 minutes

void check_watchdog(void) {
    uint32_t now = esp_timer_get_time() / 1000;
    
    if (!spa_status.connected && 
        now - spa_status.last_update_ms > SPA_WATCHDOG_TIMEOUT_MS) {
        ESP_LOGE(TAG, "Spa connection lost too long, rebooting");
        esp_restart();
    }
}
```

---

## 9. Build & Flash Instructions

### 9.1 Prerequisites

```bash
# Install ESP-IDF v5.2+
git clone -b v5.2 --recursive https://github.com/espressif/esp-idf.git
cd esp-idf
./install.sh esp32c6
source export.sh

# Install Zigbee SDK
git clone https://github.com/espressif/esp-zigbee-sdk.git
```

### 9.2 Project Setup

```bash
# Clone project
git clone https://github.com/youruser/spa-controller.git
cd spa-controller

# Configure
idf.py set-target esp32c6
idf.py menuconfig
# Set WiFi credentials, etc.

# Build
idf.py build

# Flash
idf.py -p /dev/ttyUSB0 flash monitor
```

### 9.3 Pairing with IKEA Dirigera

1. Put Dirigera hub in pairing mode (IKEA app → Add device → Other)
2. Power on ESP32-C6
3. Wait for LED to go solid (network joined)
4. Devices should appear:
   - "Spa Temperature" (sensor)
   - "Spa Light" (light)
   - "Spa Jets" (switch)

---

## 10. Project Structure

```
spa-controller/
├── CMakeLists.txt
├── sdkconfig.defaults
├── partitions.csv
├── main/
│   ├── CMakeLists.txt
│   ├── main.c
│   ├── balboa/
│   │   ├── balboa.h
│   │   ├── balboa.c
│   │   ├── protocol.h
│   │   └── protocol.c
│   ├── zigbee/
│   │   ├── zigbee.h
│   │   ├── zigbee.c
│   │   └── endpoints.c
│   ├── config/
│   │   ├── config.h
│   │   └── config.c
│   └── wifi/
│       ├── wifi.h
│       └── wifi.c
├── components/
│   └── (any custom components)
└── docs/
    └── *.md
```

---

## 11. Testing Checklist

### 11.1 Unit Tests
- [ ] CRC-8 calculation matches known values
- [ ] Status message parsing
- [ ] Temperature conversion (°F ↔ Zigbee)
- [ ] Command message construction

### 11.2 Integration Tests
- [ ] WiFi connection
- [ ] Spa discovery (UDP broadcast)
- [ ] Spa TCP connection
- [ ] Status reception
- [ ] Zigbee network join
- [ ] IKEA app device discovery
- [ ] Light control round-trip
- [ ] Pump control round-trip
- [ ] Temperature reporting

### 11.3 Safety Tests
- [ ] Pump cooldown enforced
- [ ] Temperature limits enforced
- [ ] Watchdog reboot works
- [ ] Reconnect after spa power cycle
- [ ] Reconnect after WiFi dropout

---

## 12. Future Enhancements

- [ ] Matter/Thread support (when IKEA supports it)
- [ ] Local web UI for configuration
- [ ] Energy monitoring (heating hours tracking)
- [ ] Scheduling (heat by time)
- [ ] Multiple pump control
- [ ] OTA firmware updates
- [ ] Home Assistant MQTT bridge option

---

*Document Version: 0.1.0-draft*
*Last Updated: 2025-01-21*
