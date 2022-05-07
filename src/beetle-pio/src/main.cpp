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
extern const char * redis_auth;

using bus_type = arduino::tft_spi<VSPI, LCD_SS_PIN, SPI_MODE0, (240 * 320) * 2 + 8>;
using lcd_type = arduino::ili9341v<
  PIN_NUM_DC,
  PIN_NUM_RST,
  PIN_NUM_BCKL,
  bus_type,
  LCD_ROTATION,
  LCD_BACKLIGHT_HIGH,
  400,
  200
>;

// TODO: explore constructing the wifi + redis managers here. Dealing with the copy
// and/or movement semantics of their constructors is out of scope for now.
Engine eng(std::make_pair(ap_ssid, ap_password), std::make_tuple(redis_host, redis_port, redis_auth));
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

void setup(void) {
#ifndef RELEASE
  Serial.begin(115200);
#endif

  pinMode(LED_BUILTIN, OUTPUT);
  pinMode(PIN_NUM_RST, OUTPUT);
  pinMode(PIN_NUM_DC, OUTPUT);
  pinMode(LCD_SS_PIN, OUTPUT);
  pinMode(PIN_NUM_BCKL, OUTPUT);

  digitalWrite(PIN_NUM_BCKL, LOW);

  unsigned char i = 0;

  while (i < 12) {
    digitalWrite(LED_BUILTIN, i % 2 == 0 ? HIGH : LOW);
    delay(500);
    i += 1;
  }

  if (strlen(redis_auth) > 60) {
    failed = true;

#ifndef RELEASE
    log_e("redis authentication too large");
#endif

    return;
  }

  /*
  if (!vcnl.begin()) {
#ifndef RELEASE
    log_d("unable to detect vcnl proximity sensor");
#endif

    failed = true;
    return;
  }
  */

  log_d("boot complete, redis-config. host: %s | port: %d", redis_host, redis_port);

  digitalWrite(PIN_NUM_RST, HIGH);
  delay(10);
  digitalWrite(PIN_NUM_RST, LOW);
  delay(100);
  digitalWrite(PIN_NUM_RST, HIGH);
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
  heap_debug_tick += 1;
  if (heap_debug_tick > heap_debug_tick_minimum) {
    log_d("free memory before malloc: %d", ESP.getFreeHeap());
    // uint16_t prox = vcnl.readProximity();
    // log_d("proximity: %d", prox);
  }
#endif

  last_frame = now;

  // Apply updates.
  state = eng.update(state);
  view.render(state);

#ifndef RELEASE
  if (heap_debug_tick > heap_debug_tick_minimum) {
    log_d("free memory after malloc: %d", ESP.getFreeHeap());
    heap_debug_tick = 0;
  }
#endif
}

