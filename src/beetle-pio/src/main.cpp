#include <Arduino.h>

#include <WiFi.h>
#include <WiFiClient.h>
#include <WiFiAP.h>

#include <tft_spi.hpp>
#include <gfx.hpp>

#include "ili9341v.hpp"
#include "board-layout.hpp"
#include "wifi-config.hpp"
#include "index-html.hpp"
#include "jellee_ttf.hpp"

#include "wifi-manager.hpp"

using bus_type = arduino::tft_spi_ex<3, 17, 23, -1, 18>;
using lcd_type = arduino::ili9341v<
  PIN_NUM_DC,
  PIN_NUM_RST,
  PIN_NUM_BCKL,
  bus_type,
  LCD_ROTATION,
  LCD_BACKLIGHT_HIGH
>;
using lcd_color = gfx::color<typename lcd_type::pixel_type>;

lcd_type lcd;
wifimanager::Manager wi(INDEX_HTML, std::make_pair(AP_SSID, AP_PASSWORD));

unsigned char MAX_FRAME_COUNT = 15;
unsigned char MIN_FRAME_DELAY = 200;
unsigned long last_frame = 0;
unsigned char part = 0;

void setup(void) {
  Serial.begin(9600);
  pinMode(LED_BUILTIN, OUTPUT);
  pinMode(PIN_NUM_RST, OUTPUT);
  pinMode(PIN_NUM_DC, OUTPUT);
  pinMode(LCD_SS_PIN, OUTPUT);
  pinMode(PIN_NUM_BCKL, OUTPUT);

  digitalWrite(PIN_NUM_BCKL, LOW);

  unsigned char i = 0;

  while (i < 6) {
    digitalWrite(LED_BUILTIN, i % 2 == 0 ? HIGH : LOW);
    delay(500);
    i += 1;
  }

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

  if (now - last_frame < MIN_FRAME_DELAY) {
    delay(MIN_FRAME_DELAY - (now - last_frame));
    return;
  }

  wi.frame(now);

  const gfx::open_font & f = Jellee_Bold_ttf;
  float scale = f.scale(30);

  switch (part) {
    case 0: {
      const char * text = "1: the quick brown";
      gfx::srect16 text_rect = f.measure_text((gfx::ssize16) lcd.dimensions(), {0, 0}, text, scale).bounds();
      gfx::draw::filled_rectangle(lcd, (gfx::srect16) lcd.bounds(), lcd_color::black);
      gfx::draw::text(lcd, text_rect, {0, 0}, text, f, scale, lcd_color::white, lcd_color::black, false);
      delay(1000);
      part += 1;
      break;
    }
    case 1: {
      const char * text = "2. fox jumps over";
      gfx::srect16 text_rect = f.measure_text((gfx::ssize16) lcd.dimensions(), {0, 0}, text, scale).bounds();
      gfx::draw::filled_rectangle(lcd, (gfx::srect16) lcd.bounds(), lcd_color::black);
      gfx::draw::text(lcd, text_rect, {0, 0}, text, f, scale, lcd_color::white, lcd_color::black, false);
      delay(1000);
      part += 1;
      break;
    }
    case 2: {
      const char * text = "3. the lazy dog";
      gfx::srect16 text_rect = f.measure_text((gfx::ssize16) lcd.dimensions(), {0, 0}, text, scale).bounds();
      gfx::draw::filled_rectangle(lcd, (gfx::srect16) lcd.bounds(), lcd_color::black);
      gfx::draw::text(lcd, text_rect, {0, 0}, text, f, scale, lcd_color::white, lcd_color::black, false);
      delay(1000);
      part += 1;
      break;
    }
    default:
      part = 0;
      break;
  }

#ifndef RELEASE
  Serial.print("frame at [");
  Serial.print(last_frame);
  Serial.println("]");
#endif

  last_frame = now;
}
