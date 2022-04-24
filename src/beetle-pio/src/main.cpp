#include <Arduino.h>

#include <WiFi.h>
#include <WiFiClient.h>
#include <WiFiAP.h>

#include <tft_spi.hpp>
#include <gfx.hpp>

#include "ili9341v.hpp"
#include "board-layout.hpp"
#include "index-html.hpp"
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

#ifndef WIFI_SSID
#define WIFI_SSID "orient-beetle setup"
#endif
#ifndef WIFI_PASSWORD
#define WIFI_PASSWORD "password"
#endif

const char * AP_SSID PROGMEM = WIFI_SSID;
const char * AP_PASSWORD PROGMEM = WIFI_PASSWORD;

lcd_type lcd;
wifimanager::Manager wi(INDEX_HTML, std::make_pair(AP_SSID, AP_PASSWORD));

using lcd_color = gfx::color<typename lcd_type::pixel_type>;

unsigned char MAX_FRAME_COUNT = 15;
unsigned char MIN_FRAME_DELAY = 200;

unsigned long last_frame = 0;
unsigned char frame = 0;

void setup(void) {
  Serial.begin(9600);
  pinMode(LED_BUILTIN, OUTPUT);
  pinMode(PIN_NUM_RST, OUTPUT);
  pinMode(PIN_NUM_DC, OUTPUT);
  pinMode(LCD_SS_PIN, OUTPUT);

  unsigned int i = 0;

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

  gfx::draw::filled_rectangle(lcd, (gfx::srect16)lcd.bounds(), lcd_color::black);

  wi.begin();
}

void loop(void) {
  auto now = millis();

  if (now - last_frame < MIN_FRAME_DELAY) {
    delay(20);
    return;
  }

  wi.frame(now);

#ifndef RELEASE
  Serial.print("frame at [");
  Serial.print(now);
  Serial.println("]");
#endif

  last_frame = now;

  frame += 1;
  if (frame > MAX_FRAME_COUNT) {
    frame = 0;
  }

  for (unsigned char i = 0; i < 5; i++) {
    gfx::rect16 r(0, 0, 19, 19);
    r = r.offset(i * 50, 0);

    if (frame > 0) {
      gfx::rect16 last = r.offset(0, (frame - 1) * 20);
      gfx::draw::filled_rectangle(lcd, last, lcd_color::black);
    } else {
      gfx::rect16 last = r.offset(0, MAX_FRAME_COUNT * 20);
      gfx::draw::filled_rectangle(lcd, last, lcd_color::black);
    }

    r = r.offset(0, frame * 20);
    gfx::draw::filled_rectangle(lcd, r, lcd_color::purple);
  }

  delay(1000);
}
