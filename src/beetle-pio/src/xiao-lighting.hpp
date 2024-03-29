#pragma once

#include "Adafruit_NeoPixel.h"
#include "esp32-hal-log.h"
#include "message-constants.hpp"
#include "state.hpp"

#define XIAO_NEOPIXEL_PIN D6

#ifndef XIAO_NEOPIXEL_COUNT
#define XIAO_NEOPIXEL_COUNT 10
#endif

namespace lighting {
class Lighting final {
 public:
  Lighting()
      : _override(false),
        _pixels(XIAO_NEOPIXEL_COUNT, XIAO_NEOPIXEL_PIN, NEO_GRB + NEO_KHZ800) {}
  ~Lighting() = default;

  Lighting(const Lighting&) = delete;
  Lighting& operator=(const Lighting&) = delete;

  Lighting(const Lighting&& other)
      : _override(other._override), _pixels(std::move(other._pixels)) {}

  Lighting& operator=(const Lighting&& other) {
    this->_override = other._override;
    this->_pixels = std::move(other._pixels);
    return *this;
  }

  Lighting& update(states::State& state) && {
    uint32_t color = _pixels.Color(0, 0, 0);

    if (std::holds_alternative<states::Unknown>(state)) {
      log_e("unknown lighting state");
      color = _pixels.Color(200, 0, 0);
    } else if (std::holds_alternative<states::Connecting>(state)) {
      log_e("connecting lighting state");
      color = _pixels.Color(20, 0, 100);
    } else if (std::holds_alternative<states::Connected>(state)) {
      log_e("connected lighting state");
      color = _pixels.Color(0, 100, 100);
    } else if (std::holds_alternative<states::Working>(state)) {
      log_e("has working state");
      color = _pixels.Color(0, 200, 0);
    } else if (std::holds_alternative<states::Configuring>(state)) {
      log_e("configuring lighting state");
      color = _pixels.Color(100, 100, 0);
    } else if (std::holds_alternative<states::Idle>(state)) {
      return *this;
    } else if (std::holds_alternative<states::HoldingUpdate>(state)) {
      color = _pixels.Color(0, 200, 0);
      states::HoldingUpdate* working_state =
          std::get_if<states::HoldingUpdate>(&state);

      char* prefix_match =
          strstr((char*)working_state->buffer->data(), LIGHTING_PREFIX);

      if (prefix_match == nullptr) {
        log_i("skipping non-lighting related message of size '%d'",
              working_state->size);
      } else {
        char* lighting_command =
            (char*)working_state->buffer->data() + LIGHTING_PREFIX_LEN;

        if (strcmp(lighting_command, "off") == 0) {
          log_i("turning lights off");
          _override = true;
        } else if (strcmp(lighting_command, "on") == 0) {
          log_i("turning lights on");
          _override = false;
        }
      }
    }

    setAll(color);

    return *this;
  }

  void boot(uint8_t boot_tick) {
    if (boot_tick == 0) {
      _pixels.begin();
    }

    _pixels.setBrightness(100);
    _pixels.clear();

    for (uint8_t i = 0; i < XIAO_NEOPIXEL_COUNT; i++) {
      auto color = boot_tick % 2 == 0 ? _pixels.Color(0, 150, 0)
                                      : _pixels.Color(0, 0, 150);
      _pixels.setPixelColor(i, color);
    }

    _pixels.show();
  }

 private:
  void setAll(uint32_t color) {
    log_i("doing lighting (override: %d)", _override);
    _pixels.clear();

    if (_override) {
      _pixels.show();
      return;
    }

    for (uint8_t i = 0; i < XIAO_NEOPIXEL_COUNT; i++) {
      _pixels.setPixelColor(i, color);
    }

    _pixels.show();
  }

  bool _override;
  Adafruit_NeoPixel _pixels;
};
}
