#include <Arduino.h>
#include <WiFi.h>
#include <WiFiClientSecure.h>

WiFiClientSecure _client;
bool done = false;

// Shamelessly taken from examples online.
void listNetworks() {
  Serial.println("** Scan Networks **");
  int numSsid = WiFi.scanNetworks(false, true);
  if (numSsid == -1) {
    Serial.println("Couldn't get a wifi connection");
    while (true)
      ;
  }

  Serial.print("number of available networks:");
  Serial.println(numSsid);
  for (int thisNet = 0; thisNet < numSsid; thisNet++) {
    Serial.print(thisNet);
    Serial.print(") ");
    Serial.print(WiFi.SSID(thisNet));
    Serial.print("\tSignal: ");
    Serial.print(WiFi.RSSI(thisNet));
    Serial.print(" dBm");
  }
}

std::shared_ptr<std::array<uint8_t, 1>> buffer =
    std::make_shared<std::array<uint8_t, 1>>();

void setup(void) {
  Serial.begin(115200);

  delay(5000);

  WiFi.begin();
  WiFi.disconnect();
  WiFi.mode(WIFI_STA);

  delay(1000);

  listNetworks();

  buffer->fill('\0');
  log_d("starting connection to %s:%s", WIFI_SSID, WIFI_PASSWORD);
  WiFi.setHostname("orient-beetle");
  WiFi.begin(WIFI_SSID, WIFI_PASSWORD);
}

void loop(void) {
  delay(1000);

  log_d("connected: %d", WiFi.status() == WL_CONNECTED ? 1 : 0);

  if (WiFi.status() == WL_CONNECTED) {
    if (done == false) {
      extern const uint8_t redis_root_ca[] asm(
          "_binary_embeds_redis_host_root_ca_pem_start");
      log_d("setting root ca\n%s\n", redis_root_ca);
      _client.setCACert((char *)redis_root_ca);
      int result = _client.connect(REDIS_HOST, REDIS_PORT);
      log_d("connection result: %d", result);
      done = true;
    }
  }
}
