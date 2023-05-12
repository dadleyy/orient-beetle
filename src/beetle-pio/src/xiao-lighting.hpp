#pragma once

#include "Adafruit_NeoPixel.h"
#include "state.hpp"

#define XIAO_NEOPIXEL_PIN D6

#ifndef XIAO_NEOPIXEL_COUNT
#define XIAO_NEOPIXEL_COUNT 2
#endif

namespace lighting {
  Adafruit_NeoPixel pixels(XIAO_NEOPIXEL_COUNT, XIAO_NEOPIXEL_PIN, NEO_GRB + NEO_KHZ800);

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
    }
    pixels.show();
  }
}
