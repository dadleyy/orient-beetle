#pragma once

// TODO: Implement a better strategy for sharing the "rendering api" between the
// firebeetle and
//       seeduino xiao esp32c3 implementations of this project.

#include "GxEPD2_4G_4G.h"
#include "PNGdec.h"
#include "U8g2_for_Adafruit_GFX.h"
#include "esp32-hal-log.h"
#include "message-constants.hpp"
#include "state.hpp"

#define DISPLAY_CHIP_SELECT_PIN A0
#define DISPLAY_DATA_COMMAND_PIN A1
#define DISPLAY_RESET_PIN A2
#define DISPLAY_BUSY_PIN A3

PNG png;
U8G2_FOR_ADAFRUIT_GFX fonts;
GxEPD2_4G_4G_R<GxEPD2_420, GxEPD2_420::HEIGHT> display =
    GxEPD2_420(DISPLAY_CHIP_SELECT_PIN, DISPLAY_DATA_COMMAND_PIN,
               DISPLAY_RESET_PIN, DISPLAY_BUSY_PIN);

float lum(uint8_t r, uint8_t g, uint8_t b) {
  return (0.2126 * r + 0.7152 * g + 0.0722 * b);
}

void draw_row(PNGDRAW *draw_context) {
  for (uint16_t i = 0; i < draw_context->iWidth; i++) {
    uint8_t r = *(draw_context->pPixels + (i * 4));
    uint8_t g = *(draw_context->pPixels + (i * 4) + 1);
    uint8_t b = *(draw_context->pPixels + (i * 4) + 2);

    if (png.getPixelType() == 0 && png.hasAlpha()) {
      r = g = b = *(draw_context->pPixels + (i * 2));
    } else if (png.getPixelType() == 0 && !png.hasAlpha()) {
      r = g = b = *(draw_context->pPixels + i);
    }

    float l = lum(r, g, b);

    uint16_t color = GxEPD_WHITE;
    if (l < lum(0x7b, 0x7d, 0x7b)) {
      color = GxEPD_BLACK;
    } else if (l < lum(0xc5, 0xc2, 0xc5)) {
      color = GxEPD_DARKGREY;
    } else if (l < lum(0xaa, 0xaa, 0xaa)) {
      color = GxEPD_LIGHTGREY;
    }

    display.drawPixel(i, draw_context->y, color);
  }
}

bool display_init() {
  display.init(115200, true, 2, false);
  display.setRotation(0);
  fonts.begin(display);

  uint16_t bg = GxEPD_WHITE;
  uint16_t fg = GxEPD_BLACK;

  log_i("initializing display. white is %d (%d). black is %d (%d)", bg,
        GxEPD_WHITE, fg, GxEPD_BLACK);

  fonts.setFontMode(1);
  fonts.setFontDirection(0);
  fonts.setForegroundColor(fg);
  fonts.setBackgroundColor(bg);
  fonts.setFont(u8g2_font_helvR14_tf);

  int16_t tw = fonts.getUTF8Width("hello world");
  int16_t ta = fonts.getFontAscent();
  int16_t td = fonts.getFontDescent();
  int16_t th = ta - td;
  uint16_t x = (display.width() - tw) / 2;
  uint16_t y = (display.height() - th) / 2 + ta;

  display.firstPage();
  do {
    display.fillScreen(bg);
    fonts.setCursor(x, y);
    fonts.print("hello world");
  } while (display.nextPage());

  return true;
}

void display_render_state(const states::HoldingUpdate *state, uint32_t t) {
  if (state->size <= 0) {
    return;
  }

  log_i("parsing '%d' bytes as if they were png", state->size);
  auto rc =
      png.openRAM((uint8_t *)state->buffer->data(), state->size, draw_row);

  if (rc == PNG_SUCCESS) {
    display.firstPage();
    auto width = png.getWidth(), height = png.getHeight(), bpp = png.getBpp();
    log_i("image specs: (%d x %d) | %d bpp | %d type | %d alpha", width, height,
          bpp, png.getPixelType(), png.hasAlpha());
    png.decode(NULL, 0);
    log_i("decode finished");
    display.nextPage();
    png.close();
  } else {
    log_e("unable to parse png");
  }
}

void display_render_unknown(uint32_t t) {}
