#pragma once

#include "esp32-hal-log.h"
#include "Adafruit_NeoPixel.h"
#include "message-constants.hpp"
#include "state.hpp"

#define XIAO_NEOPIXEL_PIN D6

#ifndef XIAO_NEOPIXEL_COUNT
#define XIAO_NEOPIXEL_COUNT 2
#endif

namespace lighting {
  Adafruit_NeoPixel pixels(XIAO_NEOPIXEL_COUNT, XIAO_NEOPIXEL_PIN, NEO_GRB + NEO_KHZ800);

  class Lighting final {
    public:
      Lighting(): _override(false) {}
      ~Lighting() = default;

      Lighting(const Lighting&) = delete;
      Lighting& operator=(const Lighting&) = delete;
    private:
      bool _override;
  };

  void boot(uint8_t boot_tick) {
    if (boot_tick == 0) {
      pixels.begin();
    }

    pixels.setBrightness(50);
    pixels.clear();

    for(uint8_t i = 0; i < XIAO_NEOPIXEL_COUNT; i++) {
      auto color = boot_tick % 2 == 0 ? pixels.Color(0, 150, 0) : pixels.Color(0, 0, 150);
      pixels.setPixelColor(i, color);
    }

    pixels.show();
  }

  void update(states::State &state) {
    pixels.clear();
    if (std::holds_alternative<states::Unknown>(state.active)) {
      for(uint8_t i = 0; i < XIAO_NEOPIXEL_COUNT; i++) {
        pixels.setPixelColor(i, pixels.Color(200, 0, 0));
      }
    }
    if (std::holds_alternative<states::Connecting>(state.active)) {
      for(uint8_t i = 0; i < XIAO_NEOPIXEL_COUNT; i++) {
        pixels.setPixelColor(i, pixels.Color(20, 0, 100));
      }
    }
    if (std::holds_alternative<states::Connected>(state.active)) {
      for(uint8_t i = 0; i < XIAO_NEOPIXEL_COUNT; i++) {
        pixels.setPixelColor(i, pixels.Color(0, 100, 100));
      }
    }
    if (std::holds_alternative<states::Configuring>(state.active)) {
      for(uint8_t i = 0; i < XIAO_NEOPIXEL_COUNT; i++) {
        pixels.setPixelColor(i, pixels.Color(100, 100, 0));
      }
    }
    if (std::holds_alternative<states::Working>(state.active)) {
      for(uint8_t i = 0; i < XIAO_NEOPIXEL_COUNT; i++) {
        pixels.setPixelColor(i, pixels.Color(0, 100, 0));
      }

      states::Working * working_state = std::get_if<states::Working>(&state.active);
      bool sent = false;
      for (auto message = working_state->begin(); message != working_state->end(); message++) {
        if (message->size == 0 || sent) {
          continue;
        }
        char *prefix_match = strstr(message->content, LIGHTING_PREFIX);
        if (prefix_match == nullptr) {
          log_i("skipping non-lighting related message of size '%d'", message->size);
          continue;
        }
        sent = true;

        if (strcmp(message->content + LIGHTING_PREFIX_LEN, "off") == 0) {
          log_i("turning lights off");
          for(uint8_t i = 0; i < XIAO_NEOPIXEL_COUNT; i++) {
            pixels.setPixelColor(i, pixels.Color(0, 0, 0));
          }
        } else if (strcmp(message->content + LIGHTING_PREFIX_LEN, "on") == 0) {
          log_i("turning lights on");
          for(uint8_t i = 0; i < XIAO_NEOPIXEL_COUNT; i++) {
            pixels.setPixelColor(i, pixels.Color(0, 0, 0));
          }
        }
      }
    }
    pixels.show();
  }
}
