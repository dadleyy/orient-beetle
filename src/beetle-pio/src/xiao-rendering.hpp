#include "state.hpp"
#include "GxEPD2_BW.h"
#include "U8g2_for_Adafruit_GFX.h"
#include "PNGdec.h"

#define DISPLAY_CHIP_SELECT_PIN  A0
#define DISPLAY_DATA_COMMAND_PIN A1
#define DISPLAY_RESET_PIN        A2
#define DISPLAY_BUSY_PIN         A3

PNG png;
U8G2_FOR_ADAFRUIT_GFX fonts;
GxEPD2_BW<GxEPD2_420, GxEPD2_420::HEIGHT> display = GxEPD2_420(
    DISPLAY_CHIP_SELECT_PIN,
    DISPLAY_DATA_COMMAND_PIN,
    DISPLAY_RESET_PIN,
    DISPLAY_BUSY_PIN);

bool display_init() {
  display.init();
  display.setRotation(0);
  fonts.begin(display);

  uint16_t bg = GxEPD_WHITE;
  uint16_t fg = GxEPD_BLACK;

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

void PNGDraw(PNGDRAW *pDraw) {
}

void display_render_state(const states::Working * working_state, uint32_t t) {
  bool sent = false;
  for (auto message = working_state->begin(); message != working_state->end(); message++) {
    if (message->size > 0 && !sent) {
      log_i("parsing %d bytes as if they were png", message->size);
      auto rc = png.openRAM((uint8_t *) message->content, message->size, PNGDraw);
      if (rc == PNG_SUCCESS) {
        log_i("image specs: (%d x %d), %d bpp, pixel type: %d\n", png.getWidth(), png.getHeight(), png.getBpp(), png.getPixelType());
        png.close();
      } else {
        log_e("unable to parse png");
      }
      sent = true;
    }
  }
}

void display_render_unknown(uint32_t t) {}
