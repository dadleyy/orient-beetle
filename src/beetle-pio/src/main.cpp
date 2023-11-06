#ifdef XIAO
#ifdef FIREBEETLE
static_assert(1 = 0,
              "Error! Either xiao OR firebeetle must be selected, not both.");
#endif
#endif
#ifndef XIAO
#ifndef FIREBEETLE
static_assert(1 = 0,
              "Error! Either xiao OR firebeetle must be selected, not both.");
#endif
#endif

#include <Arduino.h>
#include "esp32-hal-log.h"

#ifdef XIAO
#include <SPI.h>
#include <Wire.h>
#endif

#ifdef FIREBEETLE
#include "Adafruit_VCNL4010.h"
#include "firebeetle-rendering.hpp"
#endif
#ifdef XIAO
#include "xiao-lighting.hpp"
#include "xiao-rendering.hpp"
lighting::Lighting lights;
#endif

// Internal libraries
#include "microtim.hpp"
#include "redis-events.hpp"
#include "wifi-events.hpp"

// Configuration files
#include "redis_config.hpp"
#include "wifi_config.hpp"

#include "engine.hpp"
#include "state.hpp"

extern const char* ap_ssid;
extern const char* ap_password;

extern const char* redis_host;
extern const uint32_t redis_port;
extern const char* redis_auth_username;
extern const char* redis_auth_password;

// TODO: explore constructing the wifi + redis managers here. Dealing with the
// copy
// and/or movement semantics of their constructors is out of scope for now.
Engine eng(std::make_pair(ap_ssid, ap_password),
           std::make_shared<redisevents::RedisConfig>(
               redis_host, redis_port,
               std::make_pair(redis_auth_username, redis_auth_password)));

states::State state = states::Unknown{};

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
#ifdef XIAO
  pinMode(XIAO_NEOPIXEL_PIN, OUTPUT);
#endif

#ifndef RELEASE
  Serial.begin(115200);
#endif

  unsigned char i = 0;

  while (i < 12) {
#ifdef XIAO
    lights.boot(i);
#endif
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

  // log_i("boot complete, redis-config. host: %s | port: %d", redis_host,
  // redis_port);
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
    log_d("free memory before update: %d (max %d)", ESP.getFreeHeap(),
          ESP.getMaxAllocHeap());
  }
#endif

  // Apply updates.
  state = eng.update(std::move(state), now);

#ifdef XIAO
  lights = std::move(std::move(lights).update(state));
#endif

  if (std::get_if<states::HoldingUpdate>(&state)) {
    states::HoldingUpdate* working_state =
        std::get_if<states::HoldingUpdate>(&state);
    display_render_state(working_state, last_frame);
  } else {
    display_render_unknown(last_frame);
  }

  state = states::Idle{};

  last_frame = now;
#ifndef RELEASE
  if (print_debug_info) {
    log_d("free memory after update: %d (max %d)", ESP.getFreeHeap(),
          ESP.getMaxAllocHeap());
  }
#endif
}

