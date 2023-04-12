#include <Arduino.h>
#include <WiFi.h>
#include <WiFiClientSecure.h>

WiFiClientSecure _client;
bool done = false;

void setup(void) {
  Serial.begin(115200);
  delay(5000);

  log_d("starting connection to %s:%s", WIFI_SSID, WIFI_PASSWORD);
  WiFi.setHostname("orient-beetle");
  WiFi.begin(WIFI_SSID, WIFI_PASSWORD);
}

void loop(void) {
  delay(1000);

  log_d("connected: %d", WiFi.status() == WL_CONNECTED ? 1 : 0);

  if (WiFi.status() == WL_CONNECTED) {
    if (done == false) {
      extern const uint8_t redis_root_ca[] asm("_binary_embeds_redis_host_root_ca_pem_start");
      log_d("setting root ca\n%s\n", redis_root_ca);
      _client.setCACert((char *) redis_root_ca);
      int result = _client.connect(REDIS_HOST, REDIS_PORT);
      log_d("connection result: %d", result);
      done = true;
    }
  }
}
