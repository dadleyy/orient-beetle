#include <Arduino.h>
#include <SPI.h>

// #include "Adafruit_VCNL4010.h"
#include "DFRobot_GDL.h"
#include "board-layout.h"
#include "lcd-boot.h"

unsigned long MIN_SLEEP_TIME_DELAY = 10000;
unsigned long MIN_FRAME_DELAY = 50;
unsigned long MIN_DISPLAY_DELAY = 100;
unsigned int LINE_HEIGHT = 30;

DFRobot_ILI9341_240x320_HW_SPI tft(PIN_TFT_DC, PIN_TFT_CS, PIN_TFT_RESET);
// Adafruit_VCNL4010 vcnl;

unsigned long last_read = 0;
unsigned long last_state = false;
unsigned long sleep_start = 0;
unsigned long last_display = 0;
unsigned long last_x_position = 0;
unsigned long last_y_position = 0;
unsigned int last_rotation = 0;
unsigned int last_count = 1;

void setup(void) {
  pinMode(LED_BUILTIN, OUTPUT);
  pinMode(PIN_TFT_BL, OUTPUT);
  digitalWrite(PIN_TFT_BL, LOW);

  unsigned int i = 0;

  while (i < 6) {
    digitalWrite(LED_BUILTIN, i % 2 == 0 ? HIGH : LOW);
    delay(500);
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
  // vcnl.begin();
  tft.begin();
  // tft.setRotation(1);
  // tft.setDisplayArea(0, 0, 240, 320);

  Serial.println("display started, running init sequence");
  // LCD_Init(PIN_TFT_CS, PIN_TFT_DC);

  Serial.println("initialized, filling screen");
  last_state = true;

  tft.fillScreen(COLOR_RGB565_WHITE);
  digitalWrite(PIN_TFT_BL, HIGH);
  delay(1000);
  tft.fillScreen(COLOR_RGB565_RED);
  delay(1000);
  tft.fillScreen(COLOR_RGB565_BLUE);
  delay(1000);
  tft.fillScreen(COLOR_RGB565_WHITE);

  tft.setRotation(2);
  tft.fillRect(230, 0, 10, 320, COLOR_RGB565_BLUE);
}

void loop(void) {
  unsigned long now = millis();

  if (now - last_read < MIN_FRAME_DELAY) {
    return;
  }

  last_read = now;
  auto prox = 10000;

  if (last_state && now - last_display > MIN_DISPLAY_DELAY) {
    tft.fillRect(230, 0, 10, 320, COLOR_RGB565_BLUE);

    for (unsigned char i = 0; i < last_count; i++) {
      // Clear our last rect
      tft.fillRect(last_x_position + (i * 25), last_y_position, 20, 20, COLOR_RGB565_WHITE);

      // Color our rect
      tft.fillRect(last_x_position + (i * 25), last_y_position + 20, 20, 20, COLOR_RGB565_BLUE);
    }

    // Move
    last_y_position += 20;

    // Reset at bound
    if (last_y_position + 20 > 320) {
      last_y_position = 0;
      last_count = last_count + 1;

      if (last_count > 9) {
        last_count = 1;
      }
    }

    /*
    tft.setRotation(last_rotation);

    last_rotation = last_rotation + 1;
    if (last_rotation > 3) {
      last_rotation = 0;
    }

    tft.fillScreen(COLOR_RGB565_BLACK);
    tft.setCursor(0, 0);
    tft.setTextColor(COLOR_RGB565_WHITE);
    tft.setTextSize(3);

    tft.print("p[");
    tft.print(prox);
    tft.println("]");

    for (unsigned int i = 0; i < 10; i++) {
      tft.setCursor(0, (i * LINE_HEIGHT) + LINE_HEIGHT);
      tft.print("line num [");
      tft.print(i);
      tft.println("]");
    }
    */

    // delay(1000);
    last_display = now;
  }

  return;

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
