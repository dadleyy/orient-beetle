#include <Arduino.h>
#include <tft_spi.hpp>
#include <ili9341.hpp>
#include <gfx_cpp14.hpp>

#define LCD_SS_PIN    14
#define PIN_NUM_DC    25
#define PIN_NUM_RST   26
#define PIN_NUM_BCKL  12

#define LCD_ROTATION 2
#define LCD_BACKLIGHT_HIGH true

using namespace arduino;
using namespace gfx;

using bus_type = tft_spi<3, LCD_SS_PIN, SPI_MODE0>;
using lcd_type = ili9341<
  PIN_NUM_DC,
  PIN_NUM_RST,
  PIN_NUM_BCKL,
  bus_type,
  LCD_ROTATION,
  LCD_BACKLIGHT_HIGH
>;

lcd_type lcd;

using lcd_color = color<typename lcd_type::pixel_type>;

unsigned char frame = 0;

void setup(void) {
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

  draw::filled_rectangle(lcd, (srect16)lcd.bounds(), lcd_color::black);
}

void loop(void) {
  switch (frame) {
    case 0:
      frame = frame + 1;
      draw::filled_rectangle(lcd, (srect16)lcd.bounds(), lcd_color::green);
      break;
    case 1:
      frame = frame + 1;
      draw::filled_rectangle(lcd, (srect16)lcd.bounds(), lcd_color::red);
      break;
    case 2:
      frame = frame + 1;
      draw::filled_rectangle(lcd, (srect16)lcd.bounds(), lcd_color::yellow);
      break;
    default:
      draw::filled_rectangle(lcd, (srect16)lcd.bounds(), lcd_color::black);
      frame = 0;
      break;
  }

  delay(1000);
}
