#include <Arduino.h>

#include "Adafruit_VCNL4010.h"
#include <tft_spi.hpp>
#include <gfx.hpp>

// Internal libraries
#include "ili9341v.hpp"
#include "wifi-manager.hpp"
#include "redis-manager.hpp"

// Configuration files
#include "board_layout.hpp"
#include "wifi_config.hpp"
#include "redis_config.hpp"

#include "engine.hpp"
#include "state.hpp"
#include "view.hpp"

extern const char * ap_ssid;
extern const char * ap_password;

extern const char * redis_host;
extern const uint32_t redis_port;
extern const char * redis_auth_username;
extern const char * redis_auth_password;

using bus_type = arduino::tft_spi<VSPI, LCD_PIN_NUM_SS, SPI_MODE0, (240 * 320) * 2 + 8>;
using lcd_type = arduino::ili9341v<
  LCD_PIN_NUM_DC,
  LCD_PIN_NUM_RST,
  LCD_PIN_NUM_BCKL,
  bus_type,
  LCD_ROTATION,
  LCD_BACKLIGHT_HIGH,
  400,
  200
>;

// TODO: explore constructing the wifi + redis managers here. Dealing with the copy
// and/or movement semantics of their constructors is out of scope for now.
Engine eng(
  std::make_pair(ap_ssid, ap_password),
  std::make_tuple(redis_host, redis_port, std::make_pair(redis_auth_username, redis_auth_password))
);
View<lcd_type> view;
State state;

Adafruit_VCNL4010 vcnl;

#ifndef RELEASE
uint16_t heap_debug_tick = 0;
uint16_t heap_debug_tick_minimum = 25;
#endif

unsigned long MIN_FRAME_DELAY = 200;
unsigned long last_frame = 0;
bool failed = false;
bool prox_ready = false;

void setup(void) {
#ifndef RELEASE
  Serial.begin(115200);
#endif

  pinMode(LED_BUILTIN, OUTPUT);
  pinMode(LCD_PIN_NUM_RST, OUTPUT);
  pinMode(LCD_PIN_NUM_DC, OUTPUT);
  pinMode(LCD_PIN_NUM_SS, OUTPUT);
  pinMode(LCD_PIN_NUM_BCKL, OUTPUT);

  digitalWrite(LCD_PIN_NUM_BCKL, LOW);

  unsigned char i = 0;

  while (i < 12) {
    digitalWrite(LED_BUILTIN, i % 2 == 0 ? HIGH : LOW);
    delay(500);
    i += 1;
  }

  if (vcnl.begin()) {
#ifndef RELEASE
    log_e("unable to detect vcnl proximity sensor");
#endif
    prox_ready = true;
  }

  log_d("boot complete, redis-config. host: %s | port: %d", redis_host, redis_port);

  digitalWrite(LCD_PIN_NUM_RST, HIGH);
  delay(10);
  digitalWrite(LCD_PIN_NUM_RST, LOW);
  delay(100);
  digitalWrite(LCD_PIN_NUM_RST, HIGH);
  delay(50);

  view.clear();
  eng.begin();
}

void loop(void) {
  auto now = millis();

  if (now - last_frame < MIN_FRAME_DELAY || failed) {
    digitalWrite(LED_BUILTIN, HIGH);
    delay(MIN_FRAME_DELAY - (now - last_frame));
    digitalWrite(LED_BUILTIN, LOW);
    return;
  }

#ifndef RELEASE
  if (prox_ready && heap_debug_tick > heap_debug_tick_minimum) {
    uint16_t prox = vcnl.readProximity();
    log_d("proximity: %d", prox);
  }

  heap_debug_tick += 1;
  if (heap_debug_tick > heap_debug_tick_minimum) {
    log_d("free memory before malloc: %d (max %d)", ESP.getFreeHeap(), ESP.getMaxAllocHeap());
  }
#endif

  last_frame = now;

  // Apply updates.
  state = eng.update(state);
  view.render(state);

#ifndef RELEASE
  if (heap_debug_tick > heap_debug_tick_minimum) {
    log_d("free memory after malloc: %d (max %d)", ESP.getFreeHeap(), ESP.getMaxAllocHeap());
    heap_debug_tick = 0;
  }
#endif
}

