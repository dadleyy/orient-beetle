#ifdef XIAO
#ifdef FIREBEETLE
static_assert(1=0, "Error! Either xiao OR firebeetle must be selected, not both.");
#endif
#endif
#ifndef XIAO
#ifndef FIREBEETLE
static_assert(1=0, "Error! Either xiao OR firebeetle must be selected, not both.");
#endif
#endif

#include <Arduino.h>

#ifdef XIAO
#include <Wire.h>
#include <SPI.h>
#endif

#ifdef FIREBEETLE
#include "firebeetle-rendering.hpp"
#include "Adafruit_VCNL4010.h"
#endif
#ifdef XIAO
#include "xiao-rendering.hpp"
#endif

// Internal libraries
#include "microtim.hpp"
#include "wifi-manager.hpp"
#include "redis-manager.hpp"

// Configuration files
#include "board_layout.hpp"
#include "wifi_config.hpp"
#include "redis_config.hpp"

#include "engine.hpp"
#include "state.hpp"

extern const char * ap_ssid;
extern const char * ap_password;

extern const char * redis_host;
extern const uint32_t redis_port;
extern const char * redis_auth_username;
extern const char * redis_auth_password;

// TODO: explore constructing the wifi + redis managers here. Dealing with the copy
// and/or movement semantics of their constructors is out of scope for now.
Engine eng(
  std::make_pair(ap_ssid, ap_password),
  std::make_tuple(redis_host, redis_port, std::make_pair(redis_auth_username, redis_auth_password))
);

State state;

#ifdef FIREBEETLE
Adafruit_VCNL4010 vcnl;
#endif

#ifndef RELEASE
microtim::MicroTimer _debug_timer(5000);
#endif

microtim::MicroTimer _prox_timer(5000);
bool _prox_state = true;

uint32_t last_frame = 0;
bool failed = false;
bool prox_ready = false;

void setup(void) {
#ifndef RELEASE
  Serial.begin(115200);
#endif

  unsigned char i = 0;

  while (i < 12) {
    delay(500);
    i += 1;
  }

  failed = display_init();

#ifndef DISABLE_PROXIMITY
  if (vcnl.begin()) {
    log_d("vcnl proximity sensor detected!");
    prox_ready = true;
  } else {
    log_e("[warning] no vcnl proximity sensor detected!");
    failed = true;
  }
#else
  prox_ready = false;
  log_e("[notice] proximity functionality disabled at compile time");
#endif

  log_i("boot complete, redis-config. host: %s | port: %d", redis_host, redis_port);
  eng.begin();
}

void loop(void) {
  auto now = millis();

#ifndef DISABLE_PROXIMITY
  uint16_t prox = prox_ready ? vcnl.readProximity() : 0;
  // Proximity sensor LED on/off.
  if (prox_ready) {
    if (prox > 6000) {
      _prox_timer = std::move(microtim::MicroTimer(5000));

      if (!_prox_state) {
        log_d("turning on LED");
        digitalWrite(LCD_PIN_NUM_BCKL, HIGH);
      }

      _prox_state = true;
    }

    if (_prox_timer.update(now) == 1) {
      if (_prox_state) {
        log_d("turning off LED");
        digitalWrite(LCD_PIN_NUM_BCKL, LOW);
      }
      _prox_state = false;
    }
  }
#endif

#ifndef RELEASE
  bool print_debug_info = _debug_timer.update(now) == 1;
  if (print_debug_info) {
#ifndef DISABLE_PROXIMITY
    log_d("proximity (enabled %d): %d", prox_ready, prox);
#endif
    log_d("free memory before update: %d (max %d)", ESP.getFreeHeap(), ESP.getMaxAllocHeap());
  }
#endif

  // Apply updates.
  state = eng.update(state, now);

  if (std::get_if<WorkingState>(&state.active)) {
    WorkingState * working_state = std::get_if<WorkingState>(&state.active);
    display_render_state(working_state, last_frame);
  } else {
    display_render_unknown(last_frame);
  }

  last_frame = now;
#ifndef RELEASE
  if (print_debug_info) {
    log_d("free memory after update: %d (max %d)", ESP.getFreeHeap(), ESP.getMaxAllocHeap());
  }
#endif
}

