#include <Arduino.h>
#include <SPI.h>
#include <WiFi.h>
#include <WiFiClient.h>
#include <WiFiAP.h>

// #include "Adafruit_VCNL4010.h"
#include "DFRobot_GDL.h"
#include "board-layout.h"
#include "lcd-boot.h"
#include "index-html.h"

unsigned int SERVER_BUFFER_CAPACITY = 1024;
unsigned int MAX_CLIENT_BLANK_READS = 10;
unsigned int MAX_HEADER_SIZE = 512;

unsigned long MIN_SLEEP_TIME_DELAY = 10000;
unsigned long MIN_FRAME_DELAY = 60;
unsigned long MIN_DISPLAY_DELAY = 100;
unsigned int LINE_HEIGHT = 30;

const char* PROGMEM HEADER_DELIM = "\r\n\r\n";
const char* PROGMEM AP_SSID = "ESP32-Access-Point";
const char* PROGMEM AP_PASSWORD = "123456789";

DFRobot_ILI9341_240x320_HW_SPI tft(PIN_TFT_DC, PIN_TFT_CS, PIN_TFT_RESET);
// Adafruit_VCNL4010 vcnl;

WiFiServer server(80);

unsigned long last_frame = 0;
unsigned long last_state = false;

unsigned long last_display = 0;
unsigned long last_x_position = 0;
unsigned long last_y_position = 0;
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

  WiFi.softAP(AP_SSID, AP_PASSWORD);
  IPAddress IP = WiFi.softAPIP();
  Serial.print("AP IP address: ");
  Serial.println(IP);

  server.begin();
}

void loop(void) {
  unsigned long now = millis();

  if (now - last_frame < MIN_FRAME_DELAY) {
    delay(20);
    return;
  }

  WiFiClient client = server.available();

  if (client) {
    Serial.println("has client, yay");

    // Smol delay to help make sure we have bytes on first read
    delay(20);

    unsigned int cursor = 0;
    unsigned char noreads = 0;
    bool reading = true;
    bool head = false;

    char buffer [SERVER_BUFFER_CAPACITY] = {'\0'};
    memset(buffer, '\0', SERVER_BUFFER_CAPACITY);

    while (reading) {
      reading = client.connected()
        && cursor < SERVER_BUFFER_CAPACITY - 1
        && noreads < MAX_CLIENT_BLANK_READS
        && (head ? true : cursor < MAX_HEADER_SIZE);

      if (!reading) {
        break;
      }

      if (!client.available()) {
        noreads += 1;
        delay(50);
        continue;
      }

      noreads = 0;
      char c = client.read();
      buffer[cursor] = c;

      if (cursor >= 3 && head == false) {
        char header [5] = {'\0'};

        for (unsigned char i = 0; i < 4; i++) {
          header[i] = buffer[cursor - (3 - i)];
        }

        if (strcmp(header, HEADER_DELIM) == 0) {
          memset(buffer, '\0', SERVER_BUFFER_CAPACITY);
          head = true;
          cursor = 0;
          continue;
        }
      }

      cursor += 1;
    }

    if (strlen(buffer) == 0) {
      Serial.println("responding with index");
      client.println(INDEX_HTML);
    } else {
      Serial.println("had body - using for ssid/password");
      Serial.println(buffer);
    }

    client.stop();
  }

  last_frame = now;

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
      last_count = last_count + 2;

      if (last_count > 9) {
        last_count = 1;
      }
    }

    last_display = now;
  }
}
