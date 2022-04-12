#include <SPI.h>

#include "Adafruit_VCNL4010.h"
#include "Adafruit_GFX.h"
#include "Adafruit_ILI9341.h"

#include "board-layout.h"
#include "lcd-boot.h"

unsigned long MIN_SLEEP_TIME_DELAY = 10000;
unsigned long MIN_FRAME_DELAY = 100;
unsigned long MIN_DISPLAY_DELAY = 1000;

Adafruit_ILI9341 tft(PIN_TFT_CS, PIN_TFT_DC);
Adafruit_VCNL4010 vcnl;

unsigned long last_read = 0;
unsigned long last_state = false;
unsigned long sleep_start = 0;
unsigned long last_display = 0;

void setup(void) {
  pinMode(LED_BUILTIN, OUTPUT);
  pinMode(PIN_TFT_BL, OUTPUT);
  digitalWrite(PIN_TFT_BL, LOW);

  unsigned int i = 0;

  while (i < 5) {
    digitalWrite(LED_BUILTIN, i % 2 == 0 ? HIGH : LOW);
    delay(1000);
    i += 1;
  }

  Serial.begin(9600);
  Serial.println("booting");

  pinMode(PIN_TFT_CS, OUTPUT);
  pinMode(PIN_TFT_DC, OUTPUT);
  pinMode(PIN_TFT_RESET, OUTPUT);

  Serial.println("pin modes set, entering reset");
  digitalWrite(PIN_TFT_RESET, HIGH);
  delay(10);
  digitalWrite(PIN_TFT_RESET, LOW);
  delay(100);
  digitalWrite(PIN_TFT_RESET, HIGH);
  delay(50);

  Serial.println("reset complete, starting display"); 
  vcnl.begin();
  tft.begin();

  Serial.println("display started, running init sequence");
  LCD_Init(PIN_TFT_CS, PIN_TFT_DC);

  Serial.println("initialized, filling screen");
  last_state = true;
  tft.fillScreen(ILI9341_RED);
  digitalWrite(PIN_TFT_BL, HIGH);
}

void loop(void) {
  unsigned long now = millis();

  if (now - last_read < MIN_FRAME_DELAY) {
    return;
  }

  last_read = now;
  auto prox = vcnl.readProximity();

  if (last_state && now - last_display > MIN_DISPLAY_DELAY) {
    tft.fillScreen(ILI9341_BLACK);
    tft.setCursor(0, 0);
    tft.setTextColor(ILI9341_WHITE);
    tft.setTextSize(3);
    tft.print("prox[");
    tft.print(prox);
    tft.println("]");
    last_display = now;
  }

  if (prox > 5000) {
    sleep_start = 0;
  }

  if (prox > 5000 && !last_state) {
    Serial.println("activating display");
    digitalWrite(PIN_TFT_BL, HIGH);
    last_state = true;
  }

  if (prox <= 5000 && last_state) {
    if (sleep_start == 0) {
      Serial.println("scheduling display sleep");
      sleep_start = millis();
      return;
    }

    if (now - sleep_start > MIN_SLEEP_TIME_DELAY) {
      Serial.println("going to sleep");
      digitalWrite(PIN_TFT_BL, LOW);
      last_state = false;
    }
  }
}
