#include <Arduino.h>

#include <WiFi.h>
#include <WiFiClient.h>
#include <WiFiClientSecure.h>
#include <WiFiAP.h>

#include <tft_spi.hpp>
#include <gfx.hpp>

// Internal libraries
#include "ili9341v.hpp"
#include "wifi-manager.hpp"

// Configuration files
#include "board_layout.hpp"
#include "wifi_config.hpp"
#include "index_html.hpp"
#include "jellee_ttf.hpp"
#include "redis_config.hpp"

extern const char * index_html;

extern const char * ap_ssid;
extern const char * ap_password;

extern const char * redis_host;
extern const unsigned int redis_port;
extern const char * redis_auth;

extern const uint8_t redis_root_ca_pem_start[] asm("_binary_certs_redis_host_root_ca_pem_start");
extern const uint8_t redis_root_ca_pem_end[] asm("_binary_certs_redis_host_root_ca_pem_end");

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
wifimanager::Manager wi(index_html, std::make_pair(ap_ssid, ap_password));
WiFiClientSecure client;

unsigned char MAX_FRAME_COUNT = 15;
unsigned char MIN_FRAME_DELAY = 200;
unsigned long last_frame = 0;
unsigned char part = 0;
bool certified = false;

void setup(void) {
  Serial.begin(9600);
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

#ifndef RELEASE
  Serial.println("boot complete, redis-config:");
  Serial.println("-certificate:");
  Serial.print((char *) redis_root_ca_pem_start);
  Serial.println("-host:");
  Serial.println(redis_host);
  Serial.println("-port:");
  Serial.println(redis_port);
#endif

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

  last_frame = now;

  wi.frame(now);

  if (wi.ready() && !certified) {
#ifndef RELEASE
    Serial.println("not yet certified, starting now");
#endif
    // Small delay
    delay(100);
    certified = true;

#ifndef RELEASE
    Serial.println("setting root ca cert");
#endif

    client.setCACert((char *) redis_root_ca_pem_start);

#ifndef RELEASE
    Serial.println("attempting connection");
#endif

    int result = client.connect(redis_host, redis_port);

    // If we were unable to establish the connection, bail early.
    if (result != 1) {
      client.stop();
      certified = false;
    }

#ifndef RELEASE
    Serial.print("connection to port[");
    Serial.print(redis_port);
    Serial.print("] = ");
    Serial.print(result);
    Serial.println("");
#endif
  }

  if (wi.ready() == false && certified) {
#ifndef RELEASE
      Serial.print("wifi disconnected, clearing secure client");
#endif

    client.stop();
    certified = false;
  }

  const gfx::open_font & f = Jellee_Bold_ttf;
  float scale = f.scale(30);

  gfx::draw::filled_rectangle(lcd, (gfx::srect16) lcd.bounds(), lcd_color::black);

  if (certified) {
    const char * text = "connected";
    gfx::srect16 text_rect = f.measure_text((gfx::ssize16) lcd.dimensions(), {0, 0}, text, scale).bounds();
    gfx::draw::text(lcd, text_rect.offset(0, 50), {0, 0}, text, f, scale, lcd_color::white, lcd_color::black, false);
  } else {
    const char * text = "disconnected";
    gfx::srect16 text_rect = f.measure_text((gfx::ssize16) lcd.dimensions(), {0, 0}, text, scale).bounds();
    gfx::draw::text(lcd, text_rect.offset(0, 50), {0, 0}, text, f, scale, lcd_color::white, lcd_color::black, false);
  }

  switch (part) {
    case 0: {
      const char * text = "1: the quick brown";
      gfx::srect16 text_rect = f.measure_text((gfx::ssize16) lcd.dimensions(), {0, 0}, text, scale).bounds();
      gfx::draw::text(lcd, text_rect, {0, 0}, text, f, scale, lcd_color::white, lcd_color::black, false);
      delay(1000);
      part += 1;
      break;
    }
    case 1: {
      const char * text = "2. fox jumps over";
      gfx::srect16 text_rect = f.measure_text((gfx::ssize16) lcd.dimensions(), {0, 0}, text, scale).bounds();
      gfx::draw::text(lcd, text_rect, {0, 0}, text, f, scale, lcd_color::white, lcd_color::black, false);
      delay(1000);
      part += 1;
      break;
    }
    case 2: {
      const char * text = "3. the lazy dog";
      gfx::srect16 text_rect = f.measure_text((gfx::ssize16) lcd.dimensions(), {0, 0}, text, scale).bounds();
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
}
