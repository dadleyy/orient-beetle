#include <Arduino.h>
#include <SPI.h>
#include <Wire.h>
#include "GxEPD2_4G_4G.h"
#include "PNGdec.h"
#include "U8g2_for_Adafruit_GFX.h"
#include "esp32-hal-log.h"

#define DISPLAY_CHIP_SELECT_PIN A0
#define DISPLAY_DATA_COMMAND_PIN A1
#define DISPLAY_RESET_PIN A2
#define DISPLAY_BUSY_PIN A3

PNG png;
U8G2_FOR_ADAFRUIT_GFX fonts;
GxEPD2_4G_4G_R<GxEPD2_420, GxEPD2_420::HEIGHT> display =
    GxEPD2_420(DISPLAY_CHIP_SELECT_PIN, DISPLAY_DATA_COMMAND_PIN,
               DISPLAY_RESET_PIN, DISPLAY_BUSY_PIN);

extern const uint8_t dog_start[] asm("_binary_fixtures_dog_png_start");
extern const uint8_t dog_end[] asm("_binary_fixtures_dog_png_end");
extern const uint8_t square_start[] asm("_binary_fixtures_square_png_start");
extern const uint8_t square_end[] asm("_binary_fixtures_square_png_end");

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

void setup(void) {
  Serial.begin(115200);
  uint8_t i = 0;
  while (i < 12) {
    i++;
    delay(500);
  }
  log_i("starting...");
  display.init(115200, true, 2, false);
  display.setRotation(0);
  fonts.begin(display);

  uint16_t bg = GxEPD_WHITE;
  uint16_t fg = GxEPD_BLACK;

  log_i("blak       l=%f", lum(0x00, 0x00, 0x00));
  log_i("dark grey  l=%f", lum(0x7b, 0x7d, 0x7b));
  log_i("light grey l=%f", lum(0xc5, 0xc2, 0xc5));
  log_i("white      l=%f", lum(0xFF, 0xFF, 0xFF));

  fonts.setFontMode(1);
  fonts.setFontDirection(0);
  fonts.setForegroundColor(fg);
  fonts.setBackgroundColor(bg);
  fonts.setFont(u8g2_font_helvR14_tf);

  int16_t tw = fonts.getUTF8Width("image testing");
  int16_t ta = fonts.getFontAscent();
  int16_t td = fonts.getFontDescent();
  int16_t th = ta - td;
  uint16_t x = (display.width() - tw) / 2;
  uint16_t y = (display.height() - th) / 2 + ta;

  display.firstPage();
  do {
    display.fillScreen(bg);
    fonts.setCursor(x, y);
    fonts.print("image testing");
  } while (display.nextPage());

  auto rc =
      // png.openRAM((uint8_t *)square_start, square_end - square_start,
      // draw_row);
      png.openRAM((uint8_t *)dog_start, dog_end - dog_start, draw_row);
  if (rc == PNG_SUCCESS) {
    auto width = png.getWidth(), height = png.getHeight(), bpp = png.getBpp();
    log_i("image specs: (%d x %d) | %d bpp | alpha? %d | type %d", width,
          height, bpp, png.hasAlpha(), png.getPixelType());
    display.firstPage();
    png.decode(NULL, 0);
    display.nextPage();
    png.close();
  } else {
    log_e("not a png");
  }
}

void loop(void) {
  delay(1000);
  log_i("frame (dog image is %d bytes long) (square is %d log)",
        dog_end - dog_start, square_end - square_start);
}
