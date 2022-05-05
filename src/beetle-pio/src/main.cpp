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
#include "jellee_ttf.hpp"
#include "glyphter_ttf.hpp"
#include "redis_config.hpp"

#include "engine.hpp"

// TODO: the header files were generated from this directory using:
//
// ```
// fontgen ../../.resources/Glyphter-font/Glyphter.ttf > ./include/glyphter_ttf.hpp
// fontgen ../../.resources/Jellee_1223/TTF/Jellee-Bold.ttf > ./include/jellee_ttf.hpp
// ```
//
// The generated symbols aren't particularly nice, so we'll macro them away for now.
#define ICON_FONT _______resources_Glyphter_font_Glyphter_ttf
#define TEXT_FONT _______resources_Jellee_1223_TTF_Jellee_Bold_ttf

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
using lcd_color = gfx::color<typename lcd_type::pixel_type>;
using bmp_type = gfx::bitmap<typename lcd_type::pixel_type>;
lcd_type lcd;

wifimanager::Manager wi(std::make_pair(ap_ssid, ap_password));
redismanager::Manager red(redis_host, redis_port, redis_auth);
Engine eng;
Adafruit_VCNL4010 vcnl;

#ifndef RELEASE
uint16_t heap_debug_tick = 0;
uint16_t heap_debug_tick_minimum = 25;
#endif

unsigned long MIN_FRAME_DELAY = 200;
unsigned long last_frame = 0;
bool failed = false;

void gly_render(void) {
  const gfx::open_font & gly = ICON_FONT;
  float scale = gly.scale(20);
  gfx::size16 bmp_size(240, 30);
  uint8_t * buf = (uint8_t*) malloc(bmp_type::sizeof_buffer(bmp_size));
  bmp_type tmp(bmp_size, buf);
  gfx::draw::filled_rectangle(tmp, (gfx::srect16) lcd.bounds(), lcd_color::black);
  gfx::srect16 text_rect = gly.measure_text((gfx::ssize16) lcd.dimensions(), {0, 0}, "ABCDEFG", scale).bounds();
  gfx::draw::text(tmp, text_rect, {0, 0}, "ABCDEFG", gly, scale, lcd_color::white, lcd_color::black, false);
  gfx::draw::bitmap(lcd, (gfx::srect16) lcd.bounds().offset(0, 50), tmp, tmp.bounds().offset(0, 0));
  free(buf);
}

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

  if (!vcnl.begin()) {
#ifndef RELEASE
    log_d("unable to detect vcnl proximity sensor");
#endif

    failed = true;
    return;
  }

  log_d("boot complete, redis-config. host: %s | port: %d", redis_host, redis_port);

  digitalWrite(PIN_NUM_RST, HIGH);
  delay(10);
  digitalWrite(PIN_NUM_RST, LOW);
  delay(100);
  digitalWrite(PIN_NUM_RST, HIGH);
  delay(50);

  gfx::draw::filled_rectangle(lcd, (gfx::srect16) lcd.bounds(), lcd_color::black);
  wi.begin();
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
    uint16_t prox = vcnl.readProximity();
    log_d("proximity: %d", prox);
  }
#endif

  last_frame = now;

  // Apply updates.
  eng.update(wi, red);

  // Prepare our drawing buffer.
  const gfx::open_font & f = TEXT_FONT;

  float scale = f.scale(30);
  gfx::size16 bmp_size(240, 30);
  uint8_t * buf = (uint8_t*) malloc(bmp_type::sizeof_buffer(bmp_size));

  if(buf == nullptr) {
#ifndef RELEASE
    log_d("[warning] no memory space available, wanted: %d", bmp_type::sizeof_buffer(bmp_size));
    heap_debug_tick = 0;
#endif

    delay(1000);
    return;
  }

  bmp_type tmp(bmp_size, buf);
  gfx::draw::filled_rectangle(tmp, (gfx::srect16) lcd.bounds(), lcd_color::black);

#ifndef RELEASE
  if (heap_debug_tick > heap_debug_tick_minimum) {
    log_d("free memory after malloc: %d", ESP.getFreeHeap());
    heap_debug_tick = 0;
  }
#endif

  // Write the actual text.
  char view [256];
  memset(view, '\0', 256);
  eng.view(view, 256);
  gfx::srect16 text_rect = f.measure_text((gfx::ssize16) lcd.dimensions(), {0, 0}, view, scale).bounds();
  gfx::draw::text(tmp, text_rect.offset(0, 0), {0, 0}, view, f, scale, lcd_color::white, lcd_color::black, false);
  // Draw our buffer to the display
  gfx::draw::bitmap(lcd, (gfx::srect16) lcd.bounds(), tmp, tmp.bounds());
  free(buf);

  gly_render();
}

